#!/usr/bin/env python3
"""Bridge between the TSV catalogs in `locales/` and the PO files Weblate edits (ADR 0006).

    tools/weblate_bridge.py export <po-dir> --base <base-dir> [--root DIR]
    tools/weblate_bridge.py import <po-dir> --base <base-dir> [--root DIR] [--changes FILE]
    tools/weblate_bridge.py pr-body <changes FILE>

`<po-dir>` is a checkout of the `weblate` branch: `<component>/<component>.pot` and
`<component>/<locale>.po` per component. `<base-dir>` holds `main`'s translations as of the last
export and nothing else: `export <base-dir> --base <missing-dir> --root <tree of that main commit>`
(an empty directory before the first export). It must not be the export's own output, which also
carries Weblate changes not yet merged; those would then look unchanged and never be imported.
A Weblate change is a translation that differs from the base, so a translation changed on `main` is
never reverted by Weblate's stale copy. `--root` (default: this checkout) is the repository whose
`locales/` and `tools/plural_forms.tsv` are read and, on import, written.

- export: writes `main`'s translations, with Weblate's changes on top, into `<po-dir>`. A file
  whose entries didn't change isn't rewritten, so Weblate's own formatting stays put.
- import: writes Weblate's changes into `locales/` (only rows that differ from `main`), editing
  lines in place so untouched rows stay byte-identical, and lists them as JSON for `pr-body`.
- pr-body: the `weblate/sync` pull request's description; `common.tsv` rows under Corrections
  (ADR 0003).

Offered: `common` and the apps in COMPONENTS (after their cutover), minus `@id` rows and, in an app
component, every shared string (overrides stay out, spec 11). PO text is the TSV column with `"`
escaped; both use `\\n`, `\\t` and `\\\\`. A `@plural` row is a PO plural with `msgctxt "@plural"`;
any other context is the `msgctxt`.
"""

import argparse
import json
import re
import sys
from pathlib import Path

from sync_app import parse, render

ROOT = Path(__file__).resolve().parent.parent
# An app joins at its cutover (docs/integration.md); the value is its name in catalog headers.
COMPONENTS = {"common": "every Crafting App", "wordcraft": "WordCraft", "pdfcraft": "PdfCraft"}
PLURAL = "@plural"
ID = "@id"
MSG_NOTE = "{name} placeholders are filled in when the app runs: keep each one, in any order."


def sort_key(key):
    """`sync_app.render`'s row order."""
    return (key[1].casefold(), key[1], key[0])


def plural_table(root):
    rows = {}
    for line in (root / "tools" / "plural_forms.tsv").read_text(encoding="utf-8").split("\n"):
        if line and not line.startswith("#"):
            locale, n, expr = line.split("\t")
            rows[locale] = (int(n), expr)
    return rows


def catalogs(root):
    """{(locale, component): {key: translation}} for every offered catalog in `locales/`, columns
    as written (escaped)."""
    out = {}
    for d in sorted(p for p in (root / "locales").iterdir() if p.is_dir()):
        for component in COMPONENTS:
            path = d / f"{component}.tsv"
            if path.exists():
                out[(d.name, component)] = parse(path.read_text(encoding="utf-8"))[0]
    return out


def locales(root):
    return sorted(p.name for p in (root / "locales").iterdir() if p.is_dir())


def offered_keys(cats, component):
    """The template: every (context, source) any locale has in this catalog, sorted."""
    shared = {k for (_, c), rows in cats.items() if c == "common" for k in rows}
    keys = {k for (_, c), rows in cats.items() if c == component for k in rows if k[0] != ID}
    if component != "common":
        keys -= shared
    return sorted(keys, key=sort_key)


# --- PO text ---


def po_text(column):
    return column.replace('"', '\\"')


def tsv_column(po, problems, where):
    """A PO string's text as a TSV column; only the escapes TSV also has, plus `\\"`."""

    def unescape(m):
        if m[1] == '"':
            return '"'
        if m[1] in ("n", "t", "\\"):
            return m[0]
        problems.append(f"{where}: escape \\{m[1]} has no TSV form")
        return m[0]

    return re.sub(r"\\(.?)", unescape, po, flags=re.S)


def parse_po(text):
    """Entries as dicts: ctx (None when absent), id, plural (None when absent), strs {index: text},
    flags (set), notes (`#.` lines). Text stays PO-escaped. Obsolete (`#~`) entries are dropped."""
    entries, cur, field = [], None, None

    def fresh():
        return {"ctx": None, "id": None, "plural": None, "strs": {}, "flags": set(), "notes": [], "obsolete": False}

    def add(quoted):
        if field[0] == "strs":
            cur["strs"][field[1]] += _unquote(quoted)
        else:
            cur[field[0]] += _unquote(quoted)

    def flush():
        nonlocal cur, field
        if cur and cur["id"] is not None and not cur["obsolete"]:
            entries.append(cur)
        cur, field = None, None

    for line in text.lstrip("﻿").split("\n"):
        line = line.rstrip("\r")
        if not line.strip():
            flush()
            continue
        if line.startswith("#"):
            if cur and cur["strs"]:
                flush()
            cur = cur or fresh()
            if line.startswith("#~"):
                cur["obsolete"] = True
            elif line.startswith("#,"):
                cur["flags"] |= {f.strip() for f in line[2:].split(",") if f.strip()}
            elif line.startswith("#."):
                cur["notes"].append(line[2:].strip())
            continue
        if line.startswith('"'):
            if field is None:
                raise ValueError(f"string with no keyword: {line!r}")
            add(line)
            continue
        keyword, _, rest = line.partition(" ")
        if keyword in ("msgctxt", "msgid") and cur and cur["strs"]:
            flush()
        cur = cur or fresh()
        m = re.fullmatch(r"msgstr\[(\d+)\]", keyword)
        if keyword == "msgctxt":
            field = ("ctx",)
        elif keyword == "msgid":
            field = ("id",)
        elif keyword == "msgid_plural":
            field = ("plural",)
        elif keyword == "msgstr":
            field = ("strs", 0)
        elif m:
            field = ("strs", int(m[1]))
        else:
            raise ValueError(f"unknown PO keyword: {line!r}")
        if field[0] == "strs":
            cur["strs"][field[1]] = ""
        else:
            cur[field[0]] = ""
        add(rest)
    flush()
    return entries


def _unquote(quoted):
    quoted = quoted.strip()
    if len(quoted) < 2 or not (quoted.startswith('"') and quoted.endswith('"')):
        raise ValueError(f"bad PO string: {quoted!r}")
    return quoted[1:-1]


def entry_key(e, problems, where):
    """(context, source) as the TSV has them."""
    if e["plural"] is not None:
        return (PLURAL, tsv_column(f"{e['id']}|{e['plural']}", problems, where))
    return (tsv_column(e["ctx"] or "", problems, where), tsv_column(e["id"], problems, where))


def po_values(path, nplurals, problems):
    """{key: translation as a TSV column} for a PO file's complete, non-fuzzy entries."""
    if not path.exists():
        return {}
    out = {}
    for e in parse_po(path.read_text(encoding="utf-8")):
        if e["id"] == "" and e["ctx"] is None or "fuzzy" in e["flags"]:
            continue
        where = f"{path.parent.name}/{path.name}: {e['ctx']!r} {e['id']!r}"
        key = entry_key(e, problems, where)
        if not e["strs"] or not all(e["strs"].values()):
            continue
        forms = [tsv_column(e["strs"][i], problems, where) for i in sorted(e["strs"])]
        if e["plural"] is not None:
            if len(forms) != nplurals:
                problems.append(f"{where}: {len(forms)} plural forms, the locale has {nplurals}")
                continue
            if any("|" in f for f in forms):
                problems.append(f"{where}: a plural form contains |")
                continue
        out[key] = "|".join(forms)
    return out


def weblate_changes(po_dir, base_dir, root, problems):
    """{(component, locale): {key: (translation at the last export or None, Weblate's)}} for every
    translation Weblate changed since the last export."""
    table = plural_table(root)
    out = {}
    for component in COMPONENTS:
        for path in sorted((po_dir / component).glob("*.po")):
            locale = path.stem
            n = table.get(locale, (0, ""))[0]
            now = po_values(path, n, problems)
            base = po_values(base_dir / component / path.name, n, problems)
            changed = {k: (base.get(k), v) for k, v in now.items() if base.get(k) != v}
            if changed:
                out[(component, locale)] = changed
    return out


# --- export ---


def header(component, locale, table):
    lines = [f"Project-Id-Version: craft-i18n {component}\\n"]
    if locale:
        lines.append(f"Language: {locale}\\n")
    lines += ["MIME-Version: 1.0\\n", "Content-Type: text/plain; charset=UTF-8\\n",
              "Content-Transfer-Encoding: 8bit\\n"]
    if locale:
        n, expr = table[locale]
        lines.append(f"Plural-Forms: nplurals={n}; plural=({expr});\\n")
    return 'msgid ""\nmsgstr ""\n' + "".join(f'"{l}"\n' for l in lines)


def render_entry(key, value, nplurals, flags):
    context, source = key
    out = []
    if context == "@msg":
        out.append(f"#. {MSG_NOTE}\n")
    if flags:
        out.append(f"#, {', '.join(sorted(flags))}\n")
    if context == PLURAL:
        one, other = source.split("|", 1)
        forms = value.split("|") if value else [""] * nplurals
        out.append(f'msgctxt "{PLURAL}"\nmsgid "{po_text(one)}"\nmsgid_plural "{po_text(other)}"\n')
        out += [f'msgstr[{i}] "{po_text(f)}"\n' for i, f in enumerate(forms)]
    else:
        if context:
            out.append(f'msgctxt "{po_text(context)}"\n')
        out.append(f'msgid "{po_text(source)}"\nmsgstr "{po_text(value)}"\n')
    return "".join(out)


def semantic(text):
    """A PO file's entries without formatting, to tell a real change from a reformat."""
    return sorted((repr(e["ctx"]), e["id"], repr(e["plural"]), sorted(e["strs"].items()), sorted(e["flags"]))
                  for e in parse_po(text) if e["id"] != "")


def export(po_dir, base_dir, root):
    problems = []
    changes = weblate_changes(po_dir, base_dir, root, problems)
    if problems:
        return problems
    cats, table = catalogs(root), plural_table(root)
    for component in COMPONENTS:
        keys = offered_keys(cats, component)
        d = po_dir / component
        d.mkdir(parents=True, exist_ok=True)
        pot = header(component, None, table) + "".join(
            "\n" + render_entry(k, "", 2 if k[0] == PLURAL else 1, set()) for k in keys)
        write_if_changed(d / f"{component}.pot", pot)
        for locale in locales(root):
            n = table[locale][0]
            values = dict(cats.get((locale, component), {}))
            for key, (was, new) in changes.get((component, locale), {}).items():
                # A row changed on main since the export wins; import lists the conflict.
                if values.get(key) == was:
                    values[key] = new
                elif values.get(key) != new:
                    print(f"{component}/{locale}.po: {key} changed on main and in Weblate; "
                          "keeping main's", file=sys.stderr)
            path = d / f"{locale}.po"
            existing = parse_po(path.read_text(encoding="utf-8")) if path.exists() else []
            old = {entry_key(e, [], ""): (e["flags"], "|".join(tsv_column(e["strs"][i], [], "")
                                                                for i in sorted(e["strs"])))
                   for e in existing if e["id"] != ""}
            body = ""
            for k in keys:
                value = values.get(k, "")
                was = old.get(k)
                flags = was[0] if was and was[1] == value else set()
                body += "\n" + render_entry(k, value, n, flags)
            write_if_changed(path, header(component, locale, table) + body)
    return []


def write_if_changed(path, text):
    if path.exists() and semantic(path.read_text(encoding="utf-8")) == semantic(text):
        return
    path.write_text(text, encoding="utf-8", newline="\n")


# --- import ---


def language_name(root, locale):
    path = root / "locales" / locale / "common.tsv"
    rows = parse(path.read_text(encoding="utf-8"))[0] if path.exists() else {}
    return rows.get((ID, "language.name"), locale)


def edit_catalog(path, updates, name, locale, app):
    """Set `updates` ({key: translation}) in a catalog, changing only those lines."""
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(render(name, locale, app, updates), encoding="utf-8", newline="\n")
        return
    lines = path.read_text(encoding="utf-8").split("\n")

    def rows():
        for i, line in enumerate(lines):
            cols = line.rstrip("\r").split("\t")
            if line.strip() and not line.startswith("#") and len(cols) == 3:
                yield i, (cols[0], cols[1])

    for key, value in sorted(updates.items(), key=lambda kv: sort_key(kv[0])):
        row = f"{key[0]}\t{key[1]}\t{value}"
        hit = next((i for i, k in rows() if k == key), None)
        if hit is not None:
            lines[hit] = row + ("\r" if lines[hit].endswith("\r") else "")
            continue
        after = next((i for i, k in rows() if sort_key(k) > sort_key(key)), None)
        if after is None:
            last = max((i for i, _ in rows()), default=None)
            if last is None:  # header only: before the final newline
                last = len(lines) - 2 if lines[-1] == "" else len(lines) - 1
            lines.insert(last + 1, row)
        else:
            lines.insert(after, row)
    path.write_text("\n".join(lines), encoding="utf-8", newline="\n")


def import_(po_dir, base_dir, root):
    problems = []
    changes = weblate_changes(po_dir, base_dir, root, problems)
    if problems:
        return problems, []
    cats = catalogs(root)
    shared = {k for (_, c), rows in cats.items() if c == "common" for k in rows}
    listed = []
    for (component, locale), rows in sorted(changes.items()):
        if not (root / "locales" / locale).is_dir():
            problems.append(f"{component}/{locale}.po: no such locale in locales/")
            continue
        current = cats.get((locale, component), {})
        updates = {}
        for key, (was, value) in rows.items():
            if key[0] == ID or (component != "common" and key in shared) or current.get(key) == value:
                continue
            change = {"component": component, "locale": locale, "context": key[0], "source": key[1],
                      "old": current.get(key), "new": value}
            if current.get(key) != was:
                # Changed on main since the export: a newer correction beats an older Suggestion.
                listed.append(change | {"conflict": True})
                continue
            updates[key] = value
            listed.append(change)
        if updates:
            path = root / "locales" / locale / f"{component}.tsv"
            edit_catalog(path, updates, language_name(root, locale), locale, COMPONENTS[component])
    return problems, listed


# --- pr-body ---


def pr_body(changes):
    if not changes:
        return "No translation changes.\n"
    out = ["Translations accepted in Weblate (ADR 0006). A maintainer approves and merges once CI passes.\n"]
    conflicts = [c for c in changes if c.get("conflict")]
    changes = [c for c in changes if not c.get("conflict")]
    if conflicts:
        out.append("\n## Not imported: changed on `main` since the export\n\n"
                   "`main` keeps its text; the next export puts it back in Weblate. A Reviewer can "
                   "suggest Weblate's wording again if it is still better.\n\n")
        out += [_line(c) for c in conflicts]
    corrections = [c for c in changes if c["component"] == "common"]
    if corrections:
        out.append("\n## Corrections to shared strings (ADR 0003)\n\n"
                   "Each was accepted by the locale's Reviewer in Weblate as a Correction.\n\n")
        out += [_line(c) for c in corrections]
    rest = [c for c in changes if c["component"] != "common"]
    for locale in sorted({c["locale"] for c in rest}):
        out.append(f"\n## `{locale}`\n\n")
        out += [_line(c) for c in rest if c["locale"] == locale]
    return "".join(out)


def _line(c):
    where = f"`{c['locale']}/{c['component']}.tsv`"
    ctx = f" ({c['context']})" if c["context"] else ""
    old = "new" if c["old"] is None else f"`{c['old']}` →"
    return f"- {where}: `{c['source']}`{ctx}: {old} `{c['new']}`\n"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("export", "import"):
        p = sub.add_parser(name)
        p.add_argument("po_dir", type=Path)
        p.add_argument("--base", type=Path, required=True)
        p.add_argument("--root", type=Path, default=ROOT)
        if name == "import":
            p.add_argument("--changes", type=Path)
    p = sub.add_parser("pr-body")
    p.add_argument("changes", type=Path)
    args = ap.parse_args(argv)

    if args.cmd == "pr-body":
        sys.stdout.write(pr_body(json.loads(args.changes.read_text(encoding="utf-8"))))
        return 0
    if args.cmd == "export":
        problems, listed = export(args.po_dir, args.base, args.root), []
    else:
        problems, listed = import_(args.po_dir, args.base, args.root)
    for p in problems:
        print(p, file=sys.stderr)
    if problems:
        return 1
    if args.cmd == "import":
        text = json.dumps(listed, ensure_ascii=False, indent=1) + "\n"
        if args.changes:
            args.changes.write_text(text, encoding="utf-8")
        for c in listed:
            if c.get("conflict"):  # an Actions annotation, seen even when nothing else changed
                print(f"::warning::{c['locale']}/{c['component']}: {c['source']!r} changed on main "
                      "and in Weblate; not imported")
        print(f"{sum(not c.get('conflict') for c in listed)} translations changed", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
