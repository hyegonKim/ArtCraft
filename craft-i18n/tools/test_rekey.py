"""Tests for rekey.py: python3 -m unittest discover tools"""

import tempfile
import unittest
from pathlib import Path

import rekey
import sync_app


def catalog(locale, app, *rows):
    return sync_app.header(locale, locale, app) + "\n" + "".join("\t".join(r) + "\n" for r in rows)


class Rekey(unittest.TestCase):
    def test_rename_copy_drop(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = {
                # renamed in the app catalog, keeping its translation
                "fr/demo": [("", "A", "a"), ("", "Colour", "Couleur"), ("", "Z", "z")],
                "fr/common": [("", "Cancel", "Annuler")],
                # only shared: copied to the app under the new text, the shared row stays
                "de/common": [("", "Colour", "Farbe")],
                # renamed row identical to the shared row for the new text: dropped
                "es/demo": [("", "Colour", "Color")],
                "es/common": [("", "Color", "Color")],
            }
            for name, rows in files.items():
                path = root / "locales" / f"{name}.tsv"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(catalog(*name.split("/"), *rows), encoding="utf-8")
            changed, skipped = rekey.rekey(root, "demo", "Colour", "Color")
            self.assertEqual(skipped, [])
            # as_posix: Windows Path.str uses backslashes; the CLI reports forward slashes.
            self.assertEqual(sorted(p.as_posix() for p, _ in changed),
                             ["locales/de/demo.tsv", "locales/es/demo.tsv", "locales/fr/demo.tsv"])
            rows = lambda n: sync_app.parse((root / "locales" / f"{n}.tsv").read_text(encoding="utf-8"))
            self.assertEqual(rows("fr/demo"), ({("", "A"): "a", ("", "Color"): "Couleur", ("", "Z"): "z"}, []))
            self.assertEqual(rows("de/demo")[0], {("", "Color"): "Farbe"})
            self.assertEqual(rows("de/common")[0], {("", "Colour"): "Farbe"})
            self.assertEqual(rows("es/demo")[0], {})
            # a second run finds nothing to do
            self.assertEqual(rekey.rekey(root, "demo", "Colour", "Color"), ([], []))

    def test_refuses_a_changed_shape(self):
        with self.assertRaises(SystemExit):
            rekey.main(["demo", "Open {name}", "Open {path}"])


if __name__ == "__main__":
    unittest.main()
