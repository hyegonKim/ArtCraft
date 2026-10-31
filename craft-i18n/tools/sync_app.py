#!/usr/bin/env python3
"""Bring one Crafting App's in-repo catalogs (`i18n/*.tsv`) into `locales/`, by the migration rules.

    tools/sync_app.py <app checkout> <SHA last synced from> [--ref REF]
    tools/sync_app.py <app checkout> --check [--ref REF]

Applies what the app changed between the base and `--ref` (default HEAD): new and corrected rows go
to `locales/<locale>/<app>.tsv`, never to `common.tsv` (ADR 0003); rows the app deleted leave it.
A row the app didn't touch keeps craft-i18n's wording, so a branch older than the last sync never
reverts main. Then, as `--check` alone does (writing nothing), it checks the round trip: `locales/`
reproduces every row the app has at `--ref`; exit 1 lists those it doesn't. See docs/integration.md.
"""

import argparse
import re
import subprocess
import sys
import unicodedata
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

HEADER = r"""# {name} ({locale}) interface catalog for {app}.
# Columns, separated by a TAB:  context <TAB> English source text <TAB> translation
#   context  usually empty. `@plural`, `@id` and `@msg` mean something to the apps; any other
#            word is a label that tells apart two meanings of the same English.
#   escapes  \n  \t  \\  inside a column. `{{name}}` placeholders must appear in both columns (in any
#            order), and a trailing "…" in one needs one in the other.
#   @plural  English `one|other`; the translation lists this language's plural forms, `|`-separated.
#   Lines starting with # and blank lines are ignored. A missing entry shows in English.
#
# Clean room: every entry was written from the plain meaning of the English text in ordinary
# computing vocabulary. No other product's localisation, string table or translated UI was
# consulted or copied; add entries the same way.
"""

CATALOG = re.compile(r"(?:^|/)i18n/[^/]+\.tsv$")
RESERVED = ("@plural", "@id", "@msg")
NAME = ("@id", "language.name")
PLURAL = "@plural"
# The migration's locale renames (3b's B4); Taiwan's Traditional Chinese is plain `zh-hant`.
RENAMES = {"zh": "zh-hans", "zh-cn": "zh-hans", "zh-sg": "zh-hans", "zh-tw": "zh-hant"}
APP_RENAMES = {("photocraft", "sr"): "sr-latn"}
# ponytail: the apps' language tables as `code: "…", name: "…"` (FilmCraft's enum isn't read).
LANGUAGE = re.compile(r'code:\s*"([^"]+)",\s*name:\s*"([^"]+)"')


def header(name, locale, app):
    return HEADER.format(name=name, locale=locale, app=app)


def parse(text):
    """Raw rows {(context, source): translation}, columns still escaped; the first of a duplicate
    wins, as in the apps. Returns (rows, problems)."""
    rows, problems = {}, []
    for n, line in enumerate(text.lstrip("﻿").split("\n"), 1):
        line = line.removesuffix("\r")
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != 3:
            problems.append(f"line {n}: {len(cols)} columns")
        elif (cols[0], cols[1]) in rows:
            problems.append(f"line {n}: duplicate of an earlier row, which wins")
        else:
            rows[(cols[0], cols[1])] = cols[2]
    return rows, problems


def render(name, locale, app, rows):
    order = sorted(rows, key=lambda k: (k[1].casefold(), k[1], k[0]))
    return header(name, locale, app) + "\n" + "".join(f"{c}\t{s}\t{rows[(c, s)]}\n" for c, s in order)


def escape(col):
    """An unknown escape shows as written in the apps, so its backslash is escaped (3b's B8)."""
    return re.sub(r"\\(.?)", lambda m: m[0] if m[1] and m[1] in "nt\\" else "\\\\" + m[1], col, flags=re.S)


def placeholders(text):
    return sorted(re.findall(r"\{([^{}]*)\}", text))


def template_problem(source):
    """Why an `@msg` source can never match (the checker's `check_template`), or None."""
    literals = re.split(r"\{[^}]*\}", source)
    if "{" in literals[-1]:
        return "@msg template has { with no closing }"
    if "" in literals[1:-1]:
        return "@msg template has two placeholders in a row"
    if len(literals) > 1 and not "".join(literals):
        return "@msg template has no literal text"
    return None


def complete_plural(source, value, forms):
    """A plural row read for writing: English as one|other, the value padded to the locale's forms
    (3b's B7)."""
    if "|" not in source:
        # ponytail: English plural by adding "s" (CADCraft's `{n} commit`).
        source = f"{source}|{source}s"
    parts = value.split("|")
    if forms == 1:
        parts = parts[:1]
    elif forms and len(parts) == 1:
        parts *= forms
    return source, "|".join(parts)


def unusable_row(context, source, value):
    """Why no row can ever be written on these three columns, or None."""
    if any(unicodedata.category(c) == "Cc" or c == "﻿" for c in context + source + value):
        return "invisible character"
    if not source or not value:
        return "empty column"
    if context.startswith("@") and context not in RESERVED:
        return f"unknown context {context}"
    return None


def plural_problem(source, value, forms):
    """Why a completed plural row can't be a plain|other pair, or None."""
    english, parts = source.split("|"), value.split("|")
    if len(english) != 2 or not all(english) or placeholders(english[0]) != placeholders(english[1]):
        return "plural source is not one|other"
    if (forms and len(parts) != forms) or not all(parts):
        return f"{len(parts)} plural forms, the locale has {forms}"
    if any(placeholders(p) != placeholders(english[1]) for p in parts):
        return "placeholders differ"
    return None


def text_problem(context, source, value):
    """Why a plain or `@msg` row doesn't translate its source faithfully, or None."""
    if placeholders(source) != placeholders(value):
        return "placeholders differ"
    if source.endswith("…") != value.endswith("…"):
        return "trailing … differs"
    if context == "@msg" and (problem := template_problem(source)):
        return problem
    return None


def fix(key, value, forms):
    """The row as craft-i18n holds it, after the migration's format fixes; or (None, problem) for
    a row the checker would reject."""
    context, source, value = escape(key[0]), escape(key[1]), escape(value)
    if context == PLURAL:
        source, value = complete_plural(source, value, forms)
    if (problem := unusable_row(context, source, value)):
        return None, problem
    if context == PLURAL:
        if problem := plural_problem(source, value, forms):
            return None, problem
    elif context != "@id" and (problem := text_problem(context, source, value)):
        return None, problem
    return (context, source), value


# The options the sync's own git calls spell out. Anything else at the head of an argument would
# reach git as a fresh option: a value quoted out of a file (an ATTRIBUTION SHA, a row, a path)
# must not be able to redirect the command.
GIT_OPTIONS = {"-r", "-z", "--name-only", "-q", "-b", "--verify"}


def git(checkout, *args):
    """git in `checkout`, capturing its stdout. Option-shaped arguments are only those the sync's
    calls name, so data read from files can never speak for the command line."""
    for arg in args:
        if arg.startswith("-") and arg not in GIT_OPTIONS:
            raise ValueError(f"git: {arg!r} is not one of the options the sync passes")
    return subprocess.run(["git", "-C", str(checkout), *args], check=True, capture_output=True,
                          text=True, encoding="utf-8").stdout


def locale_name(app, code):
    code = code.lower().replace("_", "-")
    return APP_RENAMES.get((app, code), RENAMES.get(code, code))


def base_language(locale):
    return locale.split("-")[0] if "-" in locale else None


class Tree:
    """`locales/` under a craft-i18n checkout, read on demand; `write` saves what changed."""

    def __init__(self, root, app):
        self.dir = root / "locales"
        self.app = app
        self.disk = {}
        self.cache = {}

    def path(self, locale, file):
        return self.dir / locale / f"{file}.tsv"

    def get(self, locale, file):
        if (locale, file) not in self.cache:
            path = self.path(locale, file)
            rows = parse(path.read_text(encoding="utf-8"))[0] if path.exists() else None
            self.disk[(locale, file)] = rows
            self.cache[(locale, file)] = None if rows is None else dict(rows)
        return self.cache[(locale, file)]

    def rows(self, locale, file):
        return self.get(locale, file) or {}

    def set(self, locale, file, rows):
        self.get(locale, file)
        self.cache[(locale, file)] = rows

    def locales(self):
        return sorted(p.name for p in self.dir.iterdir() if p.is_dir()) if self.dir.is_dir() else []

    def resolve(self, locale, key):
        """(translation, file) the app would show: its catalog, the shared one, then the same two
        of the base language (`es` for `es-mx`). Only a region falls back, as the checker's
        `base_locale` and the runtime do: `sr-latn` never shows `sr`."""
        parent, _, region = locale.rpartition("-")
        is_region = (len(region) == 2 and region.isalpha()) or (len(region) == 3 and region.isdigit())
        chain = [locale, parent] if parent and is_region else [locale]
        for file in ((l, f) for l in chain for f in (self.app, "common")):
            if key in self.rows(*file):
                return self.rows(*file)[key], file[1]
        return None, None

    def language(self, locale):
        return self.rows(locale, "common").get(NAME)

    def forms(self, locale):
        """How many plural forms the locale's catalogs list (its base language's if none has a
        plural row); None if unknown."""
        if not locale:
            return None
        d = self.dir / locale
        files = sorted(p.stem for p in d.glob("*.tsv")) if d.is_dir() else []
        counts = Counter(v.count("|") + 1 for f in files for (c, _), v in self.rows(locale, f).items()
                         if c == PLURAL)
        return counts.most_common(1)[0][0] if counts else self.forms(base_language(locale))

    def display_name(self):
        # The first header line of a written catalog: `# <Name> (<locale>) interface catalog for <app>.`
        for locale in self.locales():
            path = self.path(locale, self.app)
            if path.exists():
                head = path.read_text(encoding="utf-8").split("\n", 1)[0]
                start, sep, name = head.rpartition(" interface catalog for ")
                if sep and start.startswith("# ") and name.endswith(".") and name[:-1]:
                    return name[:-1]
        return self.app

    def write(self, names):
        """Save every changed catalog; returns (locale, file, rows before, rows after) for each."""
        changed = []
        for (locale, file), rows in sorted(self.cache.items()):
            was = self.disk[(locale, file)]
            if rows == was:
                continue
            path = self.path(locale, file)
            if rows is None:
                path.unlink()
            else:
                name = self.language(locale) or names.get(locale) or locale
                app = "every Crafting App" if file == "common" else self.display_name()
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(render(name, locale, app, rows), encoding="utf-8")
            changed.append((locale, file, was or {}, rows or {}))
        return changed


def read_catalog(checkout, sha, app, path, tree, notes):
    """One catalog file at `sha` as `locale: {key: fixed translation}` (one locale), empty rows out
    and noted."""
    locale = locale_name(app, path.rpartition("/")[2].removesuffix(".tsv"))
    raw, problems = parse(git(checkout, "show", f"{sha}:{path}"))
    notes += [f"{path}: {p}" for p in problems]
    forms = tree.forms(locale)
    rows = {}
    for key, value in raw.items():
        fixed, value = fix(key, value, forms)
        if fixed:
            rows[fixed] = value
        else:
            notes.append(f"{path}: left out, {value}: {key[0]}\t{key[1]}")
    return locale, rows


def language_names(checkout, sha, dirs, app):
    """The language names the app's ``code: "…", name: "…"`` tables give for its catalogs' locales."""
    names = {}
    for d in dirs:
        for file in git(checkout, "ls-tree", "-z", "--name-only", f"{sha}:{d}").split("\0"):
            if file.endswith(".rs"):
                for code, name in LANGUAGE.findall(git(checkout, "show", f"{sha}:{d}/{file}")):
                    names[locale_name(app, code)] = name
    return names


def read_app(checkout, sha, app, tree, notes):
    """The app's catalogs at `sha`, {locale: {key: raw translation}} after the format fixes, and
    the language names its table gives."""
    paths = [p for p in git(checkout, "ls-tree", "-r", "-z", "--name-only", sha).split("\0") if CATALOG.search(p)]
    dirs = {p.rpartition("/")[0] for p in paths}
    if len(dirs) > 1:
        sys.exit(f"catalogs in more than one directory at {sha}: {', '.join(sorted(dirs))}")
    catalogs = {}
    for path in paths:
        locale, rows = read_catalog(checkout, sha, app, path, tree, notes)
        if locale in catalogs:
            sys.exit(f"two catalogs at {sha} are both {locale}")
        catalogs[locale] = rows
    return catalogs, language_names(checkout, sha, dirs, app)


def sync_locale(tree, locale, was, now, touched, notes):
    """One locale's catalog follows the app: dropped rows leave, foreign rows follow the app unless
    it has one exactly like another source has already."""
    if now is None:
        tree.set(locale, tree.app, None)
        notes.append(f"{locale}: the app dropped this locale; {tree.app}.tsv removed")
        return
    rows = dict(tree.rows(locale, tree.app))
    tree.set(locale, tree.app, rows)
    for key in was.keys() - now.keys():
        rows.pop(key, None)
        touched.add(key)
    for key, value in now.items():
        if was.get(key) != value:
            touched.add(key)
            rows.pop(key, None)
            if tree.resolve(locale, key)[0] != value:
                rows[key] = value


def drop_unused(tree, old, new, app):
    """Rows the app's old catalogs had and none of its catalogs at `new` has: they leave the app's
    own files everywhere; notes when a shared row becomes free of `app` (other apps may use it)."""
    gone = {k for rows in old.values() for k in rows} - {k for rows in new.values() for k in rows}
    notes = []
    for key in sorted(gone):
        for locale in tree.locales():
            if key in tree.rows(locale, app):
                tree.set(locale, app, {k: v for k, v in tree.rows(locale, app).items() if k != key})
        if any(key in tree.rows(locale, "common") for locale in tree.locales()):
            notes.append(f"shared row no longer used by {app} (other apps may use it): {key[0]}\t{key[1]}")
    return gone, notes


def language_names_still_missing(tree, names, new):
    """Notes for the locales of `new` whose language name neither `tree` nor the app's table has."""
    notes = []
    for locale in sorted(new):
        if tree.language(locale) is not None:
            continue
        if names.get(locale):
            tree.set(locale, "common", {**tree.rows(locale, "common"), NAME: names[locale]})
        else:
            notes.append(f'{locale}: no language name; add "@id\tlanguage.name\t<name>" to '
                         f"locales/{locale}/common.tsv")
    return notes


def sync(tree, old, new, names):
    """Apply the app's changes from `old` to `new` to its catalogs in `tree`. Returns notes."""
    notes = []
    touched = set()
    # Base languages first: a regional row is compared with its base language's synced result.
    for locale in sorted(old.keys() | new.keys(), key=lambda l: (l.count("-"), l)):
        sync_locale(tree, locale, old.get(locale, {}), new.get(locale), touched, notes)
    gone, gone_notes = drop_unused(tree, old, new, tree.app)
    notes += gone_notes
    keep_english(tree, new, touched - gone)
    notes += language_names_still_missing(tree, names, new)
    return notes


def keep_english(tree, new, keys):
    """Where the app translates a shared string differently in most locales both have, the locales
    it leaves the string untranslated in keep English, not another app's meaning (3b)."""
    for key in sorted(keys):
        if key[0] in (PLURAL, "@id"):
            continue
        both = [l for l, rows in new.items() if key in rows and key in tree.rows(l, "common")]
        if sum(new[l][key] != tree.rows(l, "common")[key] for l in both) * 2 <= len(both):
            continue
        for locale, rows in new.items():
            value, file = tree.resolve(locale, key)
            if key not in rows and file == "common" and value != key[1]:
                tree.set(locale, tree.app, {**tree.rows(locale, tree.app), key: key[1]})


def round_trip(tree, catalogs):
    """Every app row `locales/` doesn't reproduce."""
    return [f"{locale}: {c}\t{s}: the app has {v!r}, locales/ gives {tree.resolve(locale, (c, s))[0]!r}"
            for locale, rows in sorted(catalogs.items()) for (c, s), v in sorted(rows.items())
            if tree.resolve(locale, (c, s))[0] != v]


def app_of(args):
    """The catalog name `args` runs on: given, or the app checkout's origin remote's name."""
    if args.app:
        return args.app
    origin = git(args.checkout, "remote", "get-url", "origin").strip()
    return origin.removesuffix(".git").rpartition("/")[2]


def write_sync(tree, names, notes, checkout, base, ref, app, new):
    """The non-`--check` half of `sync_app.py`: apply the app's changes from `base` to `ref`, write the
    catalogs, and give back the rows that came out (changed only, for the round trip)."""
    merged = git(checkout, "merge-base", base, ref).strip()
    if merged != git(checkout, "rev-parse", f"{base}^{{commit}}").strip():
        print(f"{base} is not an ancestor of {ref[:12]}: from their merge base {merged}")
    old, _ = read_app(checkout, merged, app, tree, [])
    notes += sync(tree, old, new, names)
    written = []
    for locale, file, was, now in tree.write(names):
        if file == "common":
            print(f"{locale}/common.tsv: language name added")
        else:
            changed = sum(was[k] != now[k] for k in was.keys() & now.keys())
            print(f"{locale}/{file}.tsv: +{len(now.keys() - was.keys())} ~{changed} "
                  f"-{len(was.keys() - now.keys())}, {len(now)} rows")
        written.append((locale, file, was, now))
    # A row the ref didn't change keeps craft-i18n's wording, so on a branch older than the
    # last sync it may differ: reported, not a failure.
    stale = {l: {k: v for k, v in rows.items() if old.get(l, {}).get(k) == v} for l, rows in new.items()}
    new = {l: {k: v for k, v in rows.items() if k not in stale[l]} for l, rows in new.items()}
    return stale, new


def round_trip_report(tree, new, stale, notes):
    """Print the notes and the two round-trip halves (`stale` changed, `new` touched); return True
    when a changed row doesn't reproduce."""
    for note in notes:
        print(note)
    kept = round_trip(tree, stale)
    for m in kept:
        print(f"untouched, craft-i18n differs: {m}")
    mismatches = round_trip(tree, new)
    for m in mismatches:
        print(f"round trip: {m}")
    total = sum(map(len, [*new.values(), *stale.values()]))
    print(f"round trip: {total - len(mismatches) - len(kept)} rows reproduced, {len(mismatches)} not"
          + (f", {len(kept)} untouched rows differ" if kept else ""))
    return bool(mismatches)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("checkout", type=Path, help="the app's git checkout")
    p.add_argument("base", nargs="?", help="the app commit last synced from (ATTRIBUTION.md)")
    p.add_argument("--ref", default="HEAD", help="the app commit to sync (default HEAD)")
    p.add_argument("--check", action="store_true", help="only check the round trip at --ref")
    p.add_argument("--app", help="catalog name, <app>.tsv (default: the origin remote's name)")
    p.add_argument("--root", type=Path, default=ROOT, help="craft-i18n checkout (default: this one)")
    args = p.parse_args(argv)
    if not args.check and not args.base:
        p.error("the base SHA is required, unless --check")
    app = app_of(args)
    ref = git(args.checkout, "rev-parse", "--verify", f"{args.ref}^{{commit}}").strip()
    tree = Tree(args.root, app)
    notes = []
    new, names = read_app(args.checkout, ref, app, tree, notes)
    if not new:
        sys.exit(f"{app}@{ref} has no i18n/*.tsv; after cutover add rows to locales/<locale>/{app}.tsv")
    print(f"read {app}@{ref}")
    stale = {}
    if not args.check:
        stale, new = write_sync(tree, names, notes, args.checkout, args.base, ref, app, new)
    return 1 if round_trip_report(tree, new, stale, notes) else 0


if __name__ == "__main__":
    sys.exit(main())
