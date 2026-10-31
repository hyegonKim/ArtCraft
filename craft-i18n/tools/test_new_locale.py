"""Tests for new_locale.py on a small synthetic tree: python3 -m unittest discover tools"""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import new_locale
import sync_app


def write(root, locale, file, name, app, *rows):
    path = root / "locales" / locale / f"{file}.tsv"
    path.parent.mkdir(parents=True, exist_ok=True)
    body = "".join("\t".join(r) + "\n" for r in rows)
    path.write_text(sync_app.header(name, locale, app) + "\n" + body, encoding="utf-8")


class NewLocale(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        write(self.root, "fr", "common", "Français", "every Crafting App",
              ("@id", "language.name", "Français"), ("", "Save", "Enregistrer"))
        write(self.root, "de", "common", "Deutsch", "every Crafting App",
              ("@id", "language.name", "Deutsch"), ("@plural", "{n} page|{n} pages", "{n} Seite|{n} Seiten"))
        write(self.root, "fr", "wordcraft", "Français", "WordCraft",
              ("", "Save", "Sauvegarder"), ("menu", "Open", "Ouvrir"))

    def run_tool(self, *argv):
        with contextlib.redirect_stdout(io.StringIO()) as out:
            new_locale.main(list(argv), root=self.root)
        return out.getvalue()

    def read(self, file):
        return (self.root / "locales" / "eo" / f"{file}.tsv").read_text(encoding="utf-8")

    def test_starts_every_string_commented_out(self):
        out = self.run_tool("eo", "Esperanto", "wordcraft")
        common = self.read("common")
        self.assertTrue(common.startswith("# Esperanto (eo) interface catalog for every Crafting App.\n"))
        self.assertIn("# Clean room:", common)
        self.assertIn("\n@id\tlanguage.name\tEsperanto\n", common)
        self.assertIn("\n#@plural\t{n} page|{n} pages\t\n", common)
        self.assertIn("\n#\tSave\t\n", common)
        self.assertNotIn("#@id\tlanguage.name", common)
        app = self.read("wordcraft")
        self.assertTrue(app.startswith("# Esperanto (eo) interface catalog for WordCraft.\n"))
        self.assertIn("\n#menu\tOpen\t\n", app)
        self.assertNotIn("Save", app, "a key common.tsv covers stays out of the app file")
        self.assertIn("common.tsv: 2 strings", out)
        self.assertIn("wordcraft.tsv: 1 strings", out)
        rows, problems = sync_app.parse(common)
        self.assertEqual((rows, problems), ({("@id", "language.name"): "Esperanto"}, []))

    def test_never_overwrites(self):
        write(self.root, "eo", "common", "Esperanto", "every Crafting App", ("", "Save", "Konservi"))
        out = self.run_tool("eo", "Esperanto", "wordcraft")
        self.assertIn("exists, left as it is", out)
        self.assertIn("\tSave\tKonservi\n", self.read("common"))
        self.assertIn("#menu\tOpen\t", self.read("wordcraft"))

    def test_rejects_bad_input(self):
        for argv in (["pt_BR", "Português"], ["eo", "Esperanto", "nocraft"]):
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(io.StringIO()):
                self.run_tool(*argv)
        self.assertFalse((self.root / "locales" / "eo").exists())


if __name__ == "__main__":
    unittest.main()
