"""Tests for weblate_bridge.py: python3 -m unittest discover -s tools"""

import shutil
import tempfile
import unittest
from pathlib import Path

import sync_app
import weblate_bridge as wb

TABLE = "# test\nde\t2\tn != 1\nru\t3\tn%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<12 || n%100>14) ? 1 : 2\n"


def catalog(locale, app, *rows):
    body = "".join("\t".join(r) + "\n" for r in rows)
    return sync_app.header(locale, locale, app) + "\n" + body


class Bridge(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.root = self.tmp / "repo"
        (self.root / "tools").mkdir(parents=True)
        (self.root / "tools" / "plural_forms.tsv").write_text(TABLE, encoding="utf-8")
        self.write("de", "common", ("@id", "language.name", "Deutsch"), ("", "Open", "Öffnen"),
                   ("@plural", "{n} page|{n} pages", "{n} Seite|{n} Seiten"),
                   ("", 'Say "hi"\\nnow', 'Sag "hallo"\\njetzt'))
        self.write("de", "wordcraft", ("", "Open", "Aufmachen"), ("", "Bold", "Fett"),
                   ("@msg", "Couldn't open {name}: {e}", "{name} ließ sich nicht öffnen: {e}"))
        self.write("ru", "common", ("@id", "language.name", "Русский"), ("", "Open", "Открыть"))
        self.po = self.tmp / "po"
        self.base = self.tmp / "base"
        self.base.mkdir()

    def write(self, locale, component, *rows):
        path = self.root / "locales" / locale / f"{component}.tsv"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(catalog(locale, component, *rows), encoding="utf-8")

    def tsv(self, locale, component):
        return (self.root / "locales" / locale / f"{component}.tsv").read_text(encoding="utf-8")

    def export(self):
        self.assertEqual(wb.export(self.po, self.base, self.root), [])
        # As the workflow does: the next base is main as exported, without Weblate's pending changes.
        shutil.rmtree(self.base)
        self.assertEqual(wb.export(self.base, self.tmp / "none", self.root), [])

    def po_file(self, component, locale):
        return (self.po / component / f"{locale}.po").read_text(encoding="utf-8")

    def set_po(self, component, locale, old, new):
        path = self.po / component / f"{locale}.po"
        text = path.read_text(encoding="utf-8")
        self.assertEqual(text.count(old), 1, old)
        path.write_text(text.replace(old, new), encoding="utf-8")

    def test_export_offers_the_right_rows(self):
        self.export()
        common = self.po_file("common", "de")
        self.assertNotIn("language.name", common)
        self.assertIn('msgctxt "@plural"\nmsgid "{n} page"\nmsgid_plural "{n} pages"\n'
                      'msgstr[0] "{n} Seite"\nmsgstr[1] "{n} Seiten"\n', common)
        self.assertIn('msgid "Say \\"hi\\"\\nnow"\nmsgstr "Sag \\"hallo\\"\\njetzt"\n', common)
        self.assertIn("Plural-Forms: nplurals=2; plural=(n != 1);", common)
        app = self.po_file("wordcraft", "de")
        self.assertNotIn("Aufmachen", app, "an override stays out of the app component")
        self.assertIn('msgid "Bold"\nmsgstr "Fett"\n', app)
        self.assertIn(f"#. {wb.MSG_NOTE}\nmsgctxt \"@msg\"\n", app)
        # ru has no wordcraft.tsv: offered, untranslated, with ru's three plural forms in common.
        self.assertIn('msgid "Bold"\nmsgstr ""\n', self.po_file("wordcraft", "ru"))
        self.assertIn('msgstr[2] ""\n', self.po_file("common", "ru"))
        self.assertTrue((self.po / "common" / "common.pot").exists())

    def test_unchanged_round_trip_changes_nothing(self):
        before = {p: p.read_bytes() for p in (self.root / "locales").rglob("*.tsv")}
        self.export()
        problems, listed = wb.import_(self.po, self.tmp / "empty", self.root)
        self.assertEqual((problems, listed), ([], []))
        self.assertEqual({p: p.read_bytes() for p in (self.root / "locales").rglob("*.tsv")}, before)

    def test_import_writes_only_the_changed_line(self):
        self.export()
        before = self.tsv("de", "common")
        self.set_po("common", "de", 'msgstr "Öffnen"', 'msgstr "Öffne"')
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(problems, [])
        self.assertEqual(listed, [{"component": "common", "locale": "de", "context": "", "source": "Open",
                                   "old": "Öffnen", "new": "Öffne"}])
        self.assertEqual(self.tsv("de", "common"), before.replace("\tÖffnen\n", "\tÖffne\n"))

    def test_import_adds_a_new_row_and_a_new_file(self):
        self.export()
        self.set_po("wordcraft", "ru", 'msgid "Bold"\nmsgstr ""', 'msgid "Bold"\nmsgstr "Жирный"')
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(problems, [])
        self.assertEqual([c["new"] for c in listed], ["Жирный"])
        rows = sync_app.parse(self.tsv("ru", "wordcraft"))[0]
        self.assertEqual(rows, {("", "Bold"): "Жирный"})
        self.assertIn("# Русский (ru) interface catalog for WordCraft.", self.tsv("ru", "wordcraft"))

    def test_plural_and_escapes_import(self):
        self.export()
        self.set_po("common", "de", 'msgstr[0] "{n} Seite"', 'msgstr[0] "{n} Blatt"')
        self.set_po("common", "de", 'msgstr "Sag \\"hallo\\"\\njetzt"', 'msgstr "Sag \\"hi\\"\\nnun"')
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(problems, [])
        rows = sync_app.parse(self.tsv("de", "common"))[0]
        self.assertEqual(rows[("@plural", "{n} page|{n} pages")], "{n} Blatt|{n} Seiten")
        self.assertEqual(rows[("", 'Say "hi"\\nnow')], 'Sag "hi"\\nnun')
        self.assertEqual(len(listed), 2)

    def test_a_stale_weblate_copy_never_reverts_main(self):
        self.export()
        self.write("de", "wordcraft", ("", "Open", "Aufmachen"), ("", "Bold", "Halbfett"),
                   ("@msg", "Couldn't open {name}: {e}", "{name} ließ sich nicht öffnen: {e}"))
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual((problems, listed), ([], []))
        self.export()
        self.assertIn('msgstr "Halbfett"', self.po_file("wordcraft", "de"))

    def test_a_row_changed_on_both_sides_keeps_main(self):
        self.export()
        self.set_po("wordcraft", "de", 'msgstr "Fett"', 'msgstr "Dick"')
        self.write("de", "wordcraft", ("", "Open", "Aufmachen"), ("", "Bold", "Halbfett"))
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(problems, [])
        self.assertEqual([(c["new"], c.get("conflict")) for c in listed], [("Dick", True)])
        self.assertIn("\tHalbfett\n", self.tsv("de", "wordcraft"))
        self.assertIn("Not imported", wb.pr_body(listed))
        self.assertNotIn("No translation changes", wb.pr_body(listed))
        self.assertEqual(wb.export(self.po, self.base, self.root), [])
        self.assertIn('msgstr "Halbfett"', self.po_file("wordcraft", "de"))

    def test_export_keeps_weblate_changes_not_yet_merged(self):
        self.export()
        self.set_po("common", "de", 'msgstr "Öffnen"', 'msgstr "Öffne"')
        self.write("de", "wordcraft", ("", "Open", "Aufmachen"), ("", "Bold", "Halbfett"))
        self.assertEqual(wb.export(self.po, self.base, self.root), [])
        self.assertIn('msgstr "Öffne"', self.po_file("common", "de"))
        self.assertIn('msgstr "Halbfett"', self.po_file("wordcraft", "de"))

    def test_a_pending_translation_survives_unrelated_exports(self):
        self.export()
        self.set_po("common", "de", 'msgstr "Öffnen"', 'msgstr "Öffne"')
        self.write("de", "wordcraft", ("", "Open", "Aufmachen"), ("", "Bold", "Halbfett"))
        self.export()
        self.export()
        self.assertIn('msgstr "Öffne"', self.po_file("common", "de"))
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(problems, [])
        self.assertEqual([(c["old"], c["new"]) for c in listed], [("Öffnen", "Öffne")])
        self.assertIn("\tÖffne\n", self.tsv("de", "common"))
        self.export()  # after the sync PR merges, nothing is pending
        self.assertEqual(wb.import_(self.po, self.base, self.root), ([], []))

    def test_fuzzy_and_bad_plurals(self):
        self.export()
        self.set_po("common", "de", 'msgid "Open"\nmsgstr "Öffnen"', '#, fuzzy\nmsgid "Open"\nmsgstr "Auf"')
        self.set_po("common", "de", 'msgstr[1] "{n} Seiten"\n', "")
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual(listed, [])
        self.assertEqual(len(problems), 1)
        self.assertIn("1 plural forms, the locale has 2", problems[0])

    def test_shared_strings_never_land_in_an_app_catalog(self):
        self.export()
        path = self.po / "wordcraft" / "de.po"
        path.write_text(path.read_text(encoding="utf-8") + '\nmsgid "Open"\nmsgstr "Los"\n', encoding="utf-8")
        problems, listed = wb.import_(self.po, self.base, self.root)
        self.assertEqual((problems, listed), ([], []))

    def test_pr_body_lists_corrections_first(self):
        body = wb.pr_body([
            {"component": "wordcraft", "locale": "de", "context": "", "source": "Bold", "old": None, "new": "Fett"},
            {"component": "common", "locale": "de", "context": "", "source": "Open", "old": "Öffen", "new": "Öffnen"},
        ])
        self.assertLess(body.index("Corrections"), body.index("## `de`"))
        self.assertIn("`de/common.tsv`: `Open`: `Öffen` → `Öffnen`", body)
        self.assertIn("`de/wordcraft.tsv`: `Bold`: new `Fett`", body)


class RealCatalogs(unittest.TestCase):
    def test_every_offered_row_round_trips(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp, "repo")
            shutil.copytree(wb.ROOT / "locales", root / "locales")
            (root / "tools").mkdir()
            shutil.copy(wb.ROOT / "tools" / "plural_forms.tsv", root / "tools")
            po, empty = Path(tmp, "po"), Path(tmp, "empty")
            empty.mkdir()
            self.assertEqual(wb.export(po, empty, root), [])
            # Against an empty base every PO value counts as Weblate's; none may differ from main.
            problems, listed = wb.import_(po, empty, root)
            self.assertEqual(problems, [])
            self.assertEqual(listed, [])


if __name__ == "__main__":
    unittest.main()
