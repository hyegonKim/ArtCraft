#!/usr/bin/env python3
"""Generate one Datadog dashboard JSON per (METHOD, route) pair found in
storyteller-web's actix-web route registry.

Output: _metrics/datadog/dashboards/per_endpoint/<METHOD>__<sanitized-path>.json

How:
- Walk the route registry files under
  crates/service/web/storyteller_web/src/http_server/routes/ to
  enumerate every (METHOD, path) registered with actix.
- Filter out HEAD (every HEAD handler in this codebase is the dummy
  `|| HttpResponse::Ok()` boilerplate — 253 identical no-ops) and OPTIONS
  (per project convention).
- Translate each actix path template like `/v1/users/{token}/profile` into
  the Datadog tag value our middleware actually emits — Datadog tag
  normalization converts `{` → `_`, `}` → `_`, and trims any trailing `_`.
- Render a per-endpoint dashboard JSON: top-line query values (rate, p95,
  p99, 5xx rate %), latency-percentile timeseries, status-class timeseries,
  and a status-code toplist.

Idempotent re-generation: any existing per-endpoint dashboard file is
preserved if its `id` field is set (so dashboards aren't recreated each
run). The script overwrites the JSON body but keeps the `id` so that
`apply_dashboards.sh` issues PUT (update) instead of POST (create).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parents[2]
ROUTES_DIR = ROOT / "crates/service/web/storyteller_web/src/http_server/routes"
OUT_DIR    = ROOT / "_metrics/datadog/dashboards/per_endpoint"

SKIP_METHODS = {"HEAD", "OPTIONS"}

# ============================================================================
# Route extraction (lifted from /tmp/extract_routes.py and dropped here so the
# script is self-contained).
# ============================================================================

SCOPE_RE        = re.compile(r'\bweb\s*::\s*scope\s*\(\s*"([^"]+)"')
RESOURCE_RE     = re.compile(r'\bweb\s*::\s*resource\s*\(\s*"([^"]+)"')
ROUTE_METHOD_RE = re.compile(
    r'\.\s*route\s*\(\s*web\s*::\s*(get|post|put|delete|patch|head|options|trace)\s*\('
)

def strip_strings_and_comments(src: str) -> str:
  out = []
  i = 0
  n = len(src)
  while i < n:
    c = src[i]
    if c == '/' and i + 1 < n and src[i+1] == '/':
      while i < n and src[i] != '\n':
        out.append(' '); i += 1
      continue
    if c == '/' and i + 1 < n and src[i+1] == '*':
      out.append('  '); i += 2
      while i + 1 < n and not (src[i] == '*' and src[i+1] == '/'):
        out.append(' ' if src[i] != '\n' else '\n'); i += 1
      if i + 1 < n:
        out.append('  '); i += 2
      continue
    if c == '"':
      out.append(c); i += 1
      while i < n and src[i] != '"':
        if src[i] == '\\' and i + 1 < n:
          out.append(src[i]); out.append(src[i+1]); i += 2
        else:
          out.append(src[i]); i += 1
      if i < n:
        out.append('"'); i += 1
      continue
    if c == "'":
      out.append(c); i += 1
      while i < n and src[i] != "'":
        if src[i] == '\\' and i + 1 < n:
          out.append(src[i]); out.append(src[i+1]); i += 2
        else:
          out.append(src[i]); i += 1
      if i < n:
        out.append("'"); i += 1
      continue
    out.append(c); i += 1
  return ''.join(out)

def join_path(parts: Iterable[str]) -> str:
  out = ''
  for p in parts:
    if not p:
      continue
    if not p.startswith('/'):
      out += '/' + p
    else:
      out += p
  while '//' in out:
    out = out.replace('//', '/')
  return out

def parse_file(path: Path):
  src = strip_strings_and_comments(path.read_text())
  events = []
  for m in re.finditer(r'\(|\)', src):
    events.append((m.start(), 'paren', m.group(0)))
  for m in SCOPE_RE.finditer(src):
    events.append((m.start(), 'scope', m.group(1), m.end()))
  for m in RESOURCE_RE.finditer(src):
    events.append((m.start(), 'resource', m.group(1), m.end()))
  for m in ROUTE_METHOD_RE.finditer(src):
    events.append((m.start(), 'route', m.group(1).upper(), m.end()))
  events.sort(key=lambda e: (e[0], 0 if e[1] != 'paren' else 1))

  # Drop parens that fall inside a matched scope/resource/route span — they
  # were already counted (with `depth += 1` below) when we processed the
  # matched event.
  consumed_until = -1
  filtered = []
  for e in events:
    if e[1] == 'paren' and e[0] < consumed_until:
      continue
    if e[1] != 'paren':
      consumed_until = e[3]
    filtered.append(e)

  results = []
  depth = 0
  frames = []  # (depth_at_open, kind, path_fragment)
  for e in filtered:
    if e[1] == 'paren':
      if e[2] == '(':
        depth += 1
      else:
        depth -= 1
        while frames and depth < frames[-1][0]:
          frames.pop()
      continue
    if e[1] == 'scope':
      _, _, path, _ = e
      frames.append((depth, 'scope', path))
      depth += 1
      continue
    if e[1] == 'resource':
      _, _, path, _ = e
      frames.append((depth, 'resource', path))
      depth += 1
      continue
    if e[1] == 'route':
      _, _, method, _ = e
      resource = None
      scope_prefix = []
      for frame in frames:
        if frame[1] == 'scope':
          scope_prefix.append(frame[2])
        elif frame[1] == 'resource':
          resource = frame[2]
      if resource is not None:
        full = join_path(scope_prefix + [resource])
        results.append((method, full))
      depth += 2  # `.route(web::METHOD(` introduces two opens
      continue
  return results

def enumerate_routes():
  files = sorted(p for p in ROUTES_DIR.rglob('*.rs') if p.name != 'mod.rs')
  routes = set()
  for f in files:
    for method, path in parse_file(f):
      routes.add((method, path))
  return sorted(routes)

# ============================================================================
# Datadog tag normalization
# ============================================================================

def to_dd_route_tag(actix_path: str) -> str:
  """Mirror Datadog's tag-value normalization for the `route` tag value our
  metrics middleware emits.

  Datadog replaces `{` and `}` with `_`, then trims trailing `_` from the
  whole value. E.g.:
    /v1/jobs/job/{token}                  -> /v1/jobs/job/_token
    /v1/users/{username}/profile          -> /v1/users/_username_/profile
    /v1/comments/list/{etype}/{etoken}    -> /v1/comments/list/_etype_/_etoken
  """
  s = actix_path.replace('{', '_').replace('}', '_')
  return s.rstrip('_')

def safe_filename(method: str, path: str) -> str:
  """Stable, filesystem-safe filename per (method, path)."""
  # Replace anything non-[A-Za-z0-9._-] with `_`
  base = re.sub(r'[^A-Za-z0-9._-]', '_', path).strip('_')
  return f"{method.upper()}__{base}.json"

# ============================================================================
# Dashboard JSON template
# ============================================================================

def build_dashboard(method: str, actix_path: str) -> dict:
  route_tag = to_dd_route_tag(actix_path)
  method_lc = method.lower()
  # Filter fragment for queries
  flt = f"service:storyteller-web,route:{route_tag},method:{method_lc}"

  title = f"[{method.upper()}] {actix_path}"
  description = (
    f"Auto-generated per-endpoint dashboard for `{method.upper()} {actix_path}`.\n\n"
    f"Filters all metrics on `service:storyteller-web,route:{route_tag},method:{method_lc}`.\n\n"
    f"Source: `script/metrics/generate_endpoint_dashboards.py`. Do not hand-edit; "
    f"changes will be overwritten on the next regeneration."
  )

  widgets = [
    # ---- Top-line group ----
    {
      "definition": {
        "type": "group",
        "layout_type": "ordered",
        "title": "Top-line health",
        "show_title": True,
        "widgets": [
          {
            "definition": {
              "type": "query_value",
              "title": "Requests/sec",
              "title_size": "16",
              "title_align": "left",
              "precision": 2,
              "autoscale": True,
              "requests": [
                {
                  "q": f"sum:http.server.request.count{{{flt}}}.as_rate()",
                  "aggregator": "avg"
                }
              ]
            },
            "layout": {"x": 0, "y": 0, "width": 3, "height": 2}
          },
          {
            "definition": {
              "type": "query_value",
              "title": "5xx error rate (%)",
              "title_size": "16",
              "title_align": "left",
              "precision": 2,
              "autoscale": False,
              "custom_unit": "%",
              "requests": [
                {
                  "q": (
                    f"(sum:http.server.request.count{{{flt},status_class:5xx}}.as_count() "
                    f"/ sum:http.server.request.count{{{flt}}}.as_count()) * 100"
                  ),
                  "aggregator": "avg",
                  "conditional_formats": [
                    {"comparator": ">",  "value": 5.0, "palette": "white_on_red"},
                    {"comparator": ">",  "value": 1.0, "palette": "white_on_yellow"},
                    {"comparator": "<=", "value": 1.0, "palette": "white_on_green"}
                  ]
                }
              ]
            },
            "layout": {"x": 3, "y": 0, "width": 3, "height": 2}
          },
          {
            "definition": {
              "type": "query_value",
              "title": "Latency p95 (ms)",
              "title_size": "16",
              "title_align": "left",
              "precision": 0,
              "autoscale": True,
              "custom_unit": "ms",
              "requests": [
                {
                  "q": f"p95:http.server.request.duration_ms{{{flt}}}",
                  "aggregator": "avg",
                  "conditional_formats": [
                    {"comparator": ">",  "value": 1000, "palette": "white_on_red"},
                    {"comparator": ">",  "value": 250,  "palette": "white_on_yellow"},
                    {"comparator": "<=", "value": 250,  "palette": "white_on_green"}
                  ]
                }
              ]
            },
            "layout": {"x": 6, "y": 0, "width": 3, "height": 2}
          },
          {
            "definition": {
              "type": "query_value",
              "title": "Latency p99 (ms)",
              "title_size": "16",
              "title_align": "left",
              "precision": 0,
              "autoscale": True,
              "custom_unit": "ms",
              "requests": [
                {
                  "q": f"p99:http.server.request.duration_ms{{{flt}}}",
                  "aggregator": "avg"
                }
              ]
            },
            "layout": {"x": 9, "y": 0, "width": 3, "height": 2}
          }
        ]
      },
      "layout": {"x": 0, "y": 0, "width": 12, "height": 3}
    },
    # ---- Trends group ----
    {
      "definition": {
        "type": "group",
        "layout_type": "ordered",
        "title": "Trends",
        "show_title": True,
        "widgets": [
          {
            "definition": {
              "type": "timeseries",
              "title": "Requests/sec by status class",
              "title_size": "16",
              "title_align": "left",
              "show_legend": True,
              "legend_layout": "horizontal",
              "requests": [
                {
                  "q": f"sum:http.server.request.count{{{flt}}} by {{status_class}}.as_rate()",
                  "display_type": "bars",
                  "style": {"palette": "semantic", "line_type": "solid", "line_width": "normal"}
                }
              ],
              "yaxis": {"scale": "linear", "include_zero": True}
            },
            "layout": {"x": 0, "y": 0, "width": 6, "height": 4}
          },
          {
            "definition": {
              "type": "timeseries",
              "title": "Latency percentiles",
              "title_size": "16",
              "title_align": "left",
              "show_legend": True,
              "legend_layout": "horizontal",
              "requests": [
                {
                  "q": f"p50:http.server.request.duration_ms{{{flt}}}",
                  "display_type": "line",
                  "style": {"palette": "cool", "line_type": "solid", "line_width": "normal"}
                },
                {
                  "q": f"p95:http.server.request.duration_ms{{{flt}}}",
                  "display_type": "line",
                  "style": {"palette": "warm", "line_type": "solid", "line_width": "normal"}
                },
                {
                  "q": f"p99:http.server.request.duration_ms{{{flt}}}",
                  "display_type": "line",
                  "style": {"palette": "warm", "line_type": "dashed", "line_width": "normal"}
                }
              ],
              "yaxis": {"scale": "linear", "include_zero": True, "label": "ms"}
            },
            "layout": {"x": 6, "y": 0, "width": 6, "height": 4}
          }
        ]
      },
      "layout": {"x": 0, "y": 3, "width": 12, "height": 5}
    },
    # ---- Status codes ----
    {
      "definition": {
        "type": "toplist",
        "title": "Status code distribution",
        "title_size": "16",
        "title_align": "left",
        "requests": [
          {"q": f"top(sum:http.server.request.count{{{flt}}} by {{status_code}}.as_count(), 25, 'sum', 'desc')"}
        ]
      },
      "layout": {"x": 0, "y": 8, "width": 12, "height": 4}
    }
  ]

  return {
    "title": title,
    "description": description,
    "layout_type": "ordered",
    "notify_list": [],
    "reflow_type": "fixed",
    "widgets": widgets
  }

# ============================================================================
# Driver
# ============================================================================

def main():
  routes = enumerate_routes()
  if not routes:
    print("no routes extracted — check ROUTES_DIR", file=sys.stderr)
    sys.exit(1)

  OUT_DIR.mkdir(parents=True, exist_ok=True)

  written = 0
  skipped = 0
  preserved_ids = 0
  for method, path in routes:
    if method in SKIP_METHODS:
      skipped += 1
      continue
    filename = safe_filename(method, path)
    out_path = OUT_DIR / filename
    body = build_dashboard(method, path)

    # Preserve `id` if it exists so the apply script PUTs instead of POSTs.
    if out_path.exists():
      try:
        existing = json.loads(out_path.read_text())
        if 'id' in existing:
          body['id'] = existing['id']
          preserved_ids += 1
      except json.JSONDecodeError:
        pass

    out_path.write_text(json.dumps(body, indent=2) + '\n')
    written += 1

  print(f"wrote {written} dashboards to {OUT_DIR.relative_to(ROOT)}/")
  print(f"  skipped {skipped} ({', '.join(sorted(SKIP_METHODS))})")
  print(f"  preserved {preserved_ids} existing id(s) for in-place updates")

if __name__ == '__main__':
  main()
