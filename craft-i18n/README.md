craft-i18n
==========

<a href="https://sonarcloud.io/project/overview?id=storytold_craft-i18n"><img alt="Scanned by SonarCloud" src="https://img.shields.io/badge/Scanned_by-SonarCloud-4b9fd5?style=flat-square&logo=sonarqubecloud&logoColor=white"></a>

Shared translations for the storytold Crafting Apps.

The apps share a lot of strings. Keeping them here means each one is translated once, and every
app picks it up. An app can still override a shared string, or add its own, in its own catalog.

## What's here

| Path | What |
|---|---|
| `locales/<locale>/common.tsv` | Shared strings: those two or more apps use. |
| `locales/<locale>/<app>.tsv` | One app's strings, and its overrides of shared ones. |
| `docs/catalog-format.md` | The catalog format, the rules CI checks, and the header every catalog starts with. |
| `docs/integration.md` | How an app embeds the catalogs as a fully optional build input. |
| `docs/adr/` | Why TSV keyed by English, and why the lookup code lives in craft-libs. |
| `GLOSSARY.md` | The words this project uses: source text, context, catalog, override… |
| `ROADMAP.md` | Milestones and which apps have moved here. |
| `ATTRIBUTION.md` | Translators, and where migrated catalogs came from. |
| `AGENTS.md` | Instructions for coding agents, pointing at the shared craftrules standards. |
| `crates/craft-i18n-tests` | The catalog checker CI runs. |

## How the apps use it

**Fully optional, never required.** Same rule as craft-fonts: the apps don't list craft-i18n in
`Cargo.toml`. Building with `CRAFT_I18N_DIR=<absolute path to this checkout>` embeds these
catalogs; without it, the app builds and runs in English. The lookup code (parsing, fallback,
plurals, the system locale) is the `craft-i18n` crate in craft-libs. See
[`docs/integration.md`](docs/integration.md).

## Translate

Catalogs are plain text, one string per line: `context<TAB>English<TAB>translation`. To add or
fix a translation, edit `locales/<locale>/<file>.tsv` (GitHub's web editor is fine) and open a pull
request. The English is exactly what the app shows; copy it from another locale's catalog of the
same name. Partial catalogs are welcome: anything missing shows in English.

Until an app's cutover it still reads its own `i18n/*.tsv`: translate that app there, in its own
repository, and the rows reach this repo with `tools/sync_app.py`
([`docs/integration.md`](docs/integration.md)). [`ROADMAP.md`](ROADMAP.md) lists which apps have
cut over.

**Clean room:** write every entry from the plain meaning of the English text. Don't consult or copy
any other product's localisation, string table or translated interface.

The full rules (contexts, placeholders, plurals) are in
[`docs/catalog-format.md`](docs/catalog-format.md); CI explains any row it rejects, by file and
line.

## Add a language

Your language isn't in `locales/` yet? You can start it yourself. Partial work is welcome, so the
first PR can be as small as you like. If you'd rather not open a PR, [request the
language](https://github.com/storytold/craft-i18n/issues/new?template=new-language.yml) instead.

1. **Pick the locale code.** Use lowercase [BCP 47](https://www.rfc-editor.org/info/bcp47): the
   language (`eo`, `ga`, `fa`), then a script or region only when it matters (`pt-br`, `zh-hans`,
   `sr-latn`). Check `locales/` first: if the language is already there, add to that catalog
   instead.
2. **Make the starter files.** With a checkout and Python 3:

   ```sh
   tools/new_locale.py eo "Esperanto" wordcraft      # locale, its name in that language, apps
   ```

   This writes `locales/eo/common.tsv`, the strings every app shares, and
   `locales/eo/wordcraft.tsv`, the strings only that app has. Name the apps you want to translate
   (`wordcraft`, `pdfcraft`, `photocraft`, `vectorcraft`, `filmcraft`, `cadcraft`,
   `craft-launcher`). Each file starts with the header and holds every English string, commented
   out:

   ```
   @id	language.name	Esperanto
   #	Save	
   #menu	Open	
   #@plural	{n} page|{n} pages	
   ```

   No checkout? Create the same files in GitHub's web editor (**Add file → Create new file**,
   then type `locales/eo/common.tsv` as the name). Copy the header from
   [`docs/catalog-format.md`](docs/catalog-format.md#header) and add your own
   `@id	language.name	<name>` row. For the English strings, copy the rows from
   another locale's file of the same name and delete their translations.
3. **Translate.** To translate a row, delete its leading `#` and type the translation after the
   last TAB:

   ```
   	Save	Konservi
   menu	Open	Malfermi
   @plural	{n} page|{n} pages	{n} paĝo|{n} paĝoj
   ```

   Keep `{name}` placeholders and a trailing `…`. An `@plural` row lists your language's plural
   forms, `|`-separated. Rows you haven't done can stay commented out (they show in English) or be
   deleted. Do the shared strings in `common.tsv` first: every app uses them. Remember the
   **clean room** rule above.
4. **Check it** (optional, CI does the same): `cargo test --workspace`. CI names the file and line
   of any row it rejects.
5. **Credit yourself.** Add a row for your locale to [`ATTRIBUTION.md`](ATTRIBUTION.md): your
   name or handle and which app you translated. Say if you used AI tools, and whether a native
   speaker reviewed the translation.
6. **Open a pull request** here, titled like `eo: WordCraft catalog`.

An app only offers a language it lists in its own code. When it adds a new language, it also adds
the code to that list (for WordCraft, `LANGUAGES` in `crates/ui-egui/src/i18n/mod.rs`). Mention
this in your PR, and a maintainer will add the code or tell you where. Until the app is ready (a
right-to-left script, for example), your catalog is kept here and waits. A translation shows in a
release once the app's pin, `craft-i18n.rev`, reaches the commit that adds it.

## Develop

```sh
cargo test --workspace                          # check every catalog
cargo test coverage_report -- --nocapture       # rows per catalog against the largest locale
```

Code and translations are MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`).
