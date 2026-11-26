#!/usr/bin/env python3
"""Initial Rust-table migration for TextureCraft (storytold/texturecraft#419).

python tools/export_texturecraft.py ../texturecraft --ref <source SHA>
python tools/export_texturecraft.py ../texturecraft --ref <source SHA> --check

Requires Git and rustc. Rustc decodes the source tables, avoiding a second parser for Rust
string literals. This is a full initial export, not a replacement for incremental TSV sync.
Run against the pre-cutover commit. Record the printed full SHA in ATTRIBUTION.md.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

from sync_app import parse, render

ROOT = Path(__file__).resolve().parent.parent
LANGUAGES = {
    'ar': 'العربية', 'de': 'Deutsch', 'es': 'Español', 'fr': 'Français',
    'it': 'Italiano', 'ja': '日本語', 'ko': '한국어',
    'pt_br': 'Português (Brasil)', 'zh_hans': '简体中文',
}


def output(*args, cwd=None):
    """Run a command and return its stdout decoded as UTF-8."""
    return subprocess.check_output(args, cwd=cwd, text=True, encoding='utf-8')


def original_tables(checkout, revision):
    """Compile the nine tables at a fixed Git revision and return escaped TSV rows."""
    with tempfile.TemporaryDirectory() as directory:
        temp = Path(directory)
        code = [r'fn esc(s: &str) -> String { s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n") }']
        for module in LANGUAGES:
            source = output('git', 'show', f'{revision}:crates/ui-egui/src/i18n/{module}.rs', cwd=checkout)
            (temp / f'{module}.rs').write_text(source, encoding='utf-8')
            code.append(f'mod {module};')
        code.append('fn main() {')
        for module in LANGUAGES:
            code.append(f'for (source, value) in {module}::TABLE {{ println!("{module}\\t{{}}\\t{{}}", esc(source), esc(value)); }}')
        code.append('}')
        source = temp / 'export.rs'
        binary = temp / ('export.exe' if os.name == 'nt' else 'export')
        source.write_text('\n'.join(code), encoding='utf-8')
        subprocess.run(['rustc', str(source), '-o', str(binary)], check=True)
        text = output(str(binary))
    tables = {}
    for line in text.splitlines():
        module, source, value = line.split('\t')
        rows = tables.setdefault(module.replace('_', '-'), {})
        key = ('', source)
        if key in rows:
            raise ValueError(f'{module}: duplicate source {source!r}')
        rows[key] = value
    return tables


def read(path):
    """Read a catalog, returning no rows for an absent file and rejecting malformed rows."""
    if not path.exists():
        return {}
    rows, problems = parse(path.read_text(encoding='utf-8'))
    if problems:
        raise ValueError(f'{path}: {problems}')
    return rows


def resolved(root, locale):
    """App overrides shared rows; a regional locale can inherit its parent."""
    rows = {}
    parts = locale.split('-')
    if len(parts) > 1 and (len(parts[-1]) == 2 or parts[-1].isdigit()):
        rows.update(resolved(root, '-'.join(parts[:-1])))
    rows.update(read(root / 'locales' / locale / 'common.tsv'))
    rows.update(read(root / 'locales' / locale / 'texturecraft.tsv'))
    return rows


def migrate(checkout, revision, root, check=False):
    """Export an initial catalog snapshot or verify it without writes when check is true."""
    sha = output('git', 'rev-parse', '--verify', revision + '^{commit}', cwd=checkout).strip()
    tables = original_tables(checkout, sha)
    for locale, original in tables.items():
        path = root / 'locales' / locale / 'texturecraft.tsv'
        if not check:
            # Exclude only rows already supplied by shared/parent catalogs. A regional
            # app row differing from its own shared catalog must remain an override.
            inherited = {}
            parts = locale.split('-')
            if len(parts) > 1 and (len(parts[-1]) == 2 or parts[-1].isdigit()):
                inherited.update(resolved(root, '-'.join(parts[:-1])))
            inherited.update(read(path.with_name('common.tsv')))
            own = {key: value for key, value in original.items() if inherited.get(key) != value}
            name = LANGUAGES[locale.replace('-', '_')]
            path.write_text(render(name, locale, 'TextureCraft', own), encoding='utf-8', newline='\n')
        actual = resolved(root, locale)
        differences = [source for key, value in original.items() if actual.get(key) != value for source in [key[1]]]
        if differences:
            raise ValueError(f'{locale}: translations changed: {differences}')
        print(f'{locale}: {len(original)} original rows resolve identically')
    print(f'Source: texturecraft@{sha}')


def main():
    """Parse the checkout, source revision and read-only verification options."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('checkout', type=Path)
    parser.add_argument('--ref', required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    migrate(args.checkout.resolve(), args.ref, ROOT, args.check)


if __name__ == '__main__':
    main()
