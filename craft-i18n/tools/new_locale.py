#!/usr/bin/env python3
"""Start a new language: write its catalogs with every English string ready to translate.

    tools/new_locale.py <locale> "<language name, in that language>" [<app> ...]

Writes `locales/<locale>/common.tsv`, and `<app>.tsv` for each app named (`wordcraft`, `pdfcraft`…),
with the header and, in `common.tsv`, the `@id language.name` row. Below them is every English
string the other locales have in a file of that name, commented out:

    #<context><TAB><English><TAB>

Delete the `#` and type the translation after the last TAB. A row left commented shows in English,
so the catalog passes CI from the start. A file that exists is never overwritten.
"""

import argparse
import re
import sys

from sync_app import NAME, ROOT, Tree, header

# ponytail: a cheap pre-check; the checker's `check_locale_name` (full BCP 47) is the real rule.
LOCALE = re.compile(r"[a-z]{2,3}(-[a-z0-9]{2,8})*")


def english(tree, file, skip=()):
    """Every (context, source) any locale has in `<file>.tsv`, less `skip` and the language name."""
    keys = {k for locale in tree.locales() for k in tree.rows(locale, file)}
    return sorted(keys - set(skip) - {NAME}, key=lambda k: (k[1].casefold(), k[1], k[0]))


def starter(tree, locale, name, file):
    shared = english(tree, "common") if file != "common" else ()
    app = "every Crafting App" if file == "common" else Tree(tree.dir.parent, file).display_name()
    text = header(name, locale, app) + "\n"
    if file == "common":
        text += f"{NAME[0]}\t{NAME[1]}\t{name}\n"
    return text + "".join(f"#{c}\t{s}\t\n" for c, s in english(tree, file, shared))


def main(argv=None, root=ROOT):
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("locale", help="lowercase BCP 47: fr, pt-br, zh-hans")
    p.add_argument("name", help="the language's name in that language: Français")
    p.add_argument("apps", nargs="*", help="app catalogs to start too: wordcraft pdfcraft …")
    args = p.parse_args(argv)
    if not LOCALE.fullmatch(args.locale):
        sys.exit(f"{args.locale!r}: write the locale as lowercase BCP 47, e.g. fr, pt-br, zh-hans")
    tree = Tree(root, "common")
    known = {f.stem for f in tree.dir.glob("*/*.tsv")}
    for app in args.apps:
        if app not in known:
            sys.exit(f"{app!r}: no locale has a {app}.tsv; apps: {', '.join(sorted(known - {'common'}))}")
    for file in ["common", *args.apps]:
        path = tree.path(args.locale, file)
        if path.exists():
            print(f"{path.relative_to(root)}: exists, left as it is")
            continue
        text = starter(tree, args.locale, args.name, file)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        todo = sum(1 for line in text.split("\n") if line.startswith("#") and line.endswith("\t"))
        print(f"{path.relative_to(root)}: {todo} strings to translate")


if __name__ == "__main__":
    main()
