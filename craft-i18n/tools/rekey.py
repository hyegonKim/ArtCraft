#!/usr/bin/env python3
"""Follow a Crafting App's change of English source text, keeping every locale's translation.

    tools/rekey.py <app> <old source> <new source> [--context CONTEXT]

Use it when the app fixes its English without changing the meaning (a typo, punctuation,
casing). Pass both texts as the catalogs write them (`\\n` for a line break). In every locale:

- a row for the old text in `<app>.tsv` is renamed to the new text, in place;
- a row only in `common.tsv` is copied to `<app>.tsv` under the new text, because other apps
  may still use the old one (ADR 0003: the shared row stays);
- a renamed row identical to a `common.tsv` row for the new text is dropped, since CI rejects
  it as redundant.

If the meaning changed, don't use this: the translations need rewriting, not re-keying. The
placeholders and trailing "…" must match, or every translation would fail the checker.
"""

import argparse
import re
import sys
from pathlib import Path

import sync_app

ROOT = Path(__file__).resolve().parent.parent


def shape(text):
    return sync_app.placeholders(text), text.endswith("…")


def find(lines, key):
    """Index of the row for (context, source), or None. The first of a duplicate wins."""
    for i, line in enumerate(lines):
        if not line.startswith("#") and line.split("\t")[:2] == list(key):
            return i
    return None


def insert(lines, row):
    """Add `row` after the last row that sorts before it (sync_app.render's order); catalogs edited
    by hand aren't always sorted, so this keeps it near its neighbours either way."""
    order = lambda c, s: (s.casefold(), s, c)
    key = order(*row.split("\t")[:2])
    at = max((i + 1 for i, line in enumerate(lines)
              if not line.startswith("#") and line.count("\t") == 2 and order(*line.split("\t")[:2]) < key),
             default=len(lines) - (lines[-1:] == [""]))  # no earlier row: before the final newline
    lines.insert(at, row)


def read(path):
    return path.read_text(encoding="utf-8").split("\n") if path.exists() else None


def rekey(root, app, old, new, context=""):
    """Returns (changed: [(path, what)], skipped: [why])."""
    tree = sync_app.Tree(root, app)
    old_key, new_key = (context, old), (context, new)
    changed, skipped = [], []
    for locale in tree.locales():
        app_path = tree.path(locale, app)
        lines = read(app_path)
        common = tree.rows(locale, "common")
        i = None if lines is None else find(lines, old_key)
        if i is not None:
            value = lines[i].split("\t")[2]
            if find(lines, new_key) is not None:
                skipped.append(f"{locale}/{app}.tsv: already has a row for the new text; left both")
                continue
            if common.get(new_key) == value:
                del lines[i]
                what = "dropped (common.tsv has it)"
            else:
                lines[i] = f"{context}\t{new}\t{value}"  # in place: a typo fix barely moves its sort position
                what = "renamed"
        elif old_key in common:
            if new_key in common or (lines is not None and find(lines, new_key) is not None):
                continue
            if lines is None:
                name = tree.language(locale) or locale
                lines = sync_app.render(name, locale, tree.display_name(), {}).split("\n")
            insert(lines, f"{context}\t{new}\t{common[old_key]}")
            what = "copied from common.tsv"
        else:
            continue
        # ponytail: rewrites the whole file, fine for catalogs of a few thousand rows
        app_path.write_text("\n".join(lines), encoding="utf-8")
        changed.append((app_path.relative_to(root), what))
    return changed, skipped


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("app")
    p.add_argument("old")
    p.add_argument("new")
    p.add_argument("--context", default="")
    a = p.parse_args(argv)
    if not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", a.app) or a.app == "common":
        sys.exit(f"not an app catalog name: {a.app}")
    if a.old == a.new:
        sys.exit("old and new text are the same")
    if shape(a.old) != shape(a.new):
        sys.exit("the placeholders or trailing … differ: the translations need rewriting, not re-keying")
    changed, skipped = rekey(ROOT, a.app, a.old, a.new, a.context)
    for path, what in changed:
        print(f"{path.as_posix()}: {what}")
    for why in skipped:
        print(f"skipped {why}", file=sys.stderr)
    if not changed:
        print("no catalog has a row for the old text", file=sys.stderr)
    return 1 if skipped or not changed else 0


if __name__ == "__main__":
    sys.exit(main())
