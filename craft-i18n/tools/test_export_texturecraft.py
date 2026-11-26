import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import export_texturecraft as export


class ExportTests(unittest.TestCase):
    def test_rustc_decodes_escaped_and_raw_literals(self):
        """Verify rustc preserves decoded Unicode, escaped characters and raw literals."""
        source = r'''pub const TABLE: &[(&str, &str)] = &[
            ("A\nB\tC\\D\"", "á\u{1f43e}"),
            (r#"raw\text"# , "translation"),
        ];'''
        real_output = export.output

        def output(*args, cwd=None):
            """Substitute Git input while still running the real compiler and exporter."""
            return source if args[0] == 'git' else real_output(*args, cwd=cwd)

        with patch.object(export, 'output', side_effect=output):
            tables = export.original_tables(Path('.'), 'fixture')
        self.assertEqual(len(tables), 9)
        self.assertEqual(tables['es'][('', r'A\nB\tC\\D"')], 'á🐾')
        self.assertEqual(tables['es'][('', r'raw\\text')], 'translation')

    def test_duplicate_keys_are_rejected(self):
        """Reject ambiguous duplicate keys instead of silently dropping a translation."""
        source = 'pub const TABLE: &[(&str, &str)] = &[("Same", "a"), ("Same", "b")];'
        real_output = export.output

        def output(*args, cwd=None):
            """Substitute Git input while still running the real compiler and exporter."""
            return source if args[0] == 'git' else real_output(*args, cwd=cwd)

        with patch.object(export, 'output', side_effect=output):
            with self.assertRaisesRegex(ValueError, 'duplicate source'):
                export.original_tables(Path('.'), 'fixture')

    def test_app_overrides_shared_and_region_inherits_parent(self):
        """Check regional fallback and app-over-shared precedence together."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for locale in ['pt', 'pt-br']:
                (root / 'locales' / locale).mkdir(parents=True)
            (root/'locales/pt/common.tsv').write_text('\tOpen\tAbrir\n', encoding='utf-8')
            (root/'locales/pt/texturecraft.tsv').write_text('\tSave\tGuardar\n', encoding='utf-8')
            (root/'locales/pt-br/common.tsv').write_text('\tSave\tSalvar\n', encoding='utf-8')
            (root/'locales/pt-br/texturecraft.tsv').write_text('\tSave\tSalvar projeto\n', encoding='utf-8')
            self.assertEqual(export.resolved(root, 'pt-br'), {('', 'Open'): 'Abrir', ('', 'Save'): 'Salvar projeto'})

    def test_script_variant_does_not_inherit_base_language(self):
        """Keep script variants separate rather than inheriting another writing system."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'locales/zh').mkdir(parents=True)
            (root/'locales/zh/common.tsv').write_text('\tOpen\tbase\n', encoding='utf-8')
            self.assertEqual(export.resolved(root, 'zh-hans'), {})


if __name__ == '__main__':
    unittest.main()
