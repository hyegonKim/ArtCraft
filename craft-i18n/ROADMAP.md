# craft-i18n roadmap

> **Last updated:** 2026-11-08 · **Change:** minor (3b: PhotoCraft cutover started)

Shared translations for the storytold Crafting Apps: one catalog per locale for shared strings,
one per app, checked in CI and embedded by each app at build time. Vocabulary is in
[`GLOSSARY.md`](GLOSSARY.md); decisions are in [`docs/adr/`](docs/adr/).

## Milestones

| # | Milestone | Where | Status |
|---|---|---|---|
| 1 | TSV catalog format and strict checker; Fluent and the unusable library crate removed; translator and integration docs | this repo | done (PR #1) |
| 2 | `craft-i18n` runtime crate: parser, lookup, fallback, plurals (`intl_pluralrules`, `icu_plurals` since craft-libs#210), locale matching, pseudo locale | craft-libs | done: merged upstream as [storytold/craft-libs#167](https://github.com/storytold/craft-libs/pull/167) (2026-10-18) |
| 3a | Migrate WordCraft and PdfCraft (proves the runtime on the largest catalogs; seeds the shared catalog) | apps | WordCraft cut over (2026-10-24); PdfCraft synced at `pdfcraft@ecdd1e6`, wiring in progress |
| 3b | Migrate the other TSV apps: PhotoCraft, VectorCraft, FilmCraft, CADCraft, craft-launcher | apps | catalogs moved here (shared catalog grows additively, [ADR 0003](docs/adr/0003-shared-catalog-grows-additively.md)); CADCraft resynced at `cadcraft@c4d9e6a`; PhotoCraft synced at `photocraft@2772ac1` (cutover started, 2026-11-08); app wiring next for the rest |
| 3c | Convert and migrate FigCraft (TOML), LightCraft (JSON), DesignCraft, GridCraft, EffectCraft (Rust tables) | apps | not started |
| 3d | Add translation to DeckCraft and SoundCraft | apps | not started |
| 4 | Language data: spelling dictionaries, hyphenation patterns and the suite word list; English embedded, other languages pinned and downloaded ([ADR 0004](docs/adr/0004-third-party-language-data.md), [ADR 0005](docs/adr/0005-language-data-lives-here.md)) | this repo, craft-libs, apps | licence table signed off 2026-11-01 (Martin Ball); phase 1 done; phase 2 (SCOWL 2026 en-US/en-GB/en-CA/en-AU dictionaries) added; phase 3 (pack manifest, checks, release workflow) done; phase 3 follow-up (75 of 80 locales publishable, 2 hidden, 3 Excluded); the last 14 need craft-proof at `47c9f625` (craft-libs #497 merged) |

Each app migration: confirm the app is MIT OR Apache-2.0, move its catalogs here (lowercase BCP 47
names, origin recorded in `ATTRIBUTION.md`), add the `build.rs` recipe from
[`docs/integration.md`](docs/integration.md), switch to the runtime crate, delete the app's copied
i18n module, and add the app-side stale/missing string report.

Known migration fixes: PdfCraft's Turkish `@plural` rows listed one form, CLDR (and CI) need two;
done in 3a by repeating the form (Turkish nouns stay singular after numerals). PdfCraft's Hindi
`@plural` rows (14, added after 3a) also listed one form; the cutover sync repeated it, so Hindi
shows what it does today. The checker takes each locale's plural form count from the runtime crate
itself (a dependency pinned by git rev; the crate's own CLDR 48.2.1 data, on `icu_plurals` since
storytold/craft-libs#210), so the two can't disagree; `tests/runtime_agreement.rs` also feeds the
crate every catalog's `@plural` rows.

Done in 3b, format only (no wording changes): Hungarian and Turkish `@plural` rows with one form
got it repeated (PhotoCraft, VectorCraft); VectorCraft's Korean and CADCraft's Chinese kept one
form; CADCraft's `\P` is escaped as `\\P`. CADCraft's `@plural` sources were the singular only;
they are now `one|other`, so **CADCraft's `count()` must look plurals up by `one|other`** when it
is wired. PhotoCraft's `sr` catalog is Latin script and moved to `sr-latn`.

Known homograph, not yet fixed: WordCraft's Paragraph Spacing menu has an "Open" preset
(`crates/ui-egui/src/ribbon.rs`, the spacing next to Tight and Relaxed). It shares the key
`("", "Open")` with File > Open, so every locale shows the verb (`es` Abrir, `de` Öffnen). The fix is
app-side in WordCraft's 3a wiring: give the preset a context (`spacing`) and add rows with context `spacing` to
`wordcraft.tsv`. Found by FredApps/craftlingo#1.

## Related work

aiongg's language-support plan, [storytold/craft-libs#198](https://github.com/storytold/craft-libs/issues/198)
(fonts in [#197](https://github.com/storytold/craft-libs/issues/197)), extends the runtime crate
(scripts, isolation, number formatting, file sizes, sorting) and lists moving the apps onto
craft-i18n as this roadmap's milestones 3a–3d. Its pilots delete PdfCraft's and CADCraft's own
Arabic reordering; each of the two is wired once it draws catalogs in logical order on the patched
egui. PdfCraft got there first (2026-11-03, storytold/egui 6d518fc) and is wired now; CADCraft still waits.

Known clash for 3c: aiongg's DesignCraft pilot rewrites the app's 1,010 right-to-left call sites,
and 3c converts DesignCraft's Rust tables. The order is decided when 3c starts.

## Apps

As surveyed on 2026-10-16 (VectorCraft recounted 2026-10-18). Locale counts exclude English.

| App | Format today | Locales | Plurals | Migrated |
|---|---|---|---|---|
| WordCraft | TSV | 22 | none | catalogs |
| PdfCraft | TSV | 22 | `@plural` | catalogs |
| PhotoCraft | TSV | 20 | `@plural` | catalogs |
| VectorCraft | TSV | 22 | `@plural` | catalogs |
| FilmCraft | TSV | 11 | none | catalogs |
| CADCraft | TSV | 12 | `@plural` + own rules | catalogs |
| craft-launcher | TSV | 10 | `@plural` | catalogs |
| FigCraft | TOML | 14 | CLDR | no |
| LightCraft | JSON | 13 | minimal | no |
| DesignCraft | Rust tables | 11 | own rules | no |
| GridCraft | Rust `match` | 9 | none | no |
| EffectCraft | Rust tables | 10 | none | no |
| DeckCraft | none | 0 | – | no |
| SoundCraft | none | 0 | – | no |

## Open decisions

- **Release builds** (maintainer): whether craft-common-actions' release workflow checks out
  craft-i18n at a pinned ref and sets `CRAFT_I18N_DIR` and `CRAFT_I18N_REQUIRED=1`, as it does for
  craft-fonts. Until then, releases are English-only unless an app sets them itself.
- **Repository home**: transferred to storytold/craft-i18n (2026-10-21).

## Change log

| Date | Change | Summary |
|---|---|---|
| 2026-10-16 | major | First roadmap: milestones 1–3d, per-app survey, open decisions |
| 2026-10-17 | minor | Milestones 1 and 2 done; WordCraft and PdfCraft catalogs migrated (196 shared strings, 28 locales) |
| 2026-10-17 | patch | Milestone 2 runtime crate opened upstream as storytold/craft-libs#167 |
| 2026-10-18 | patch | Recorded WordCraft's "Open" spacing preset translated as the verb |
| 2026-10-18 | minor | Milestone 3b catalogs migrated: PhotoCraft, VectorCraft, FilmCraft, CADCraft, craft-launcher (1279 shared strings, 32 locales) |
| 2026-10-18 | patch | Milestone 2 done: storytold/craft-libs#167 merged after review; app wiring unblocked |
| 2026-10-22 | patch | Repository transferred to storytold/craft-i18n |
| 2026-10-22 | minor | Related work: storytold/craft-libs#198; PdfCraft and CADCraft wired last; plural engine follow-up; DesignCraft clash noted for 3c |
| 2026-11-01 | minor | Milestone 4 phase 3: Language pack manifest (80 locales), checks, load-and-check per pack with craft-proof, release workflow (`docs/language-data-format.md`) |
| 2026-11-01 | patch | ADR 0004 licence table signed off (Martin Ball, 2026-11-01): the LGPL/MPL/LPPL/GPL switch is retired; uk Excluded pending its NC source data |
| 2026-11-01 | minor | Milestone 4 phase 2: SCOWL `rel-2026.02.25` Hunspell dictionaries for en-US, en-GB (-ise), en-CA and en-AU in `language-data/`, unmodified, with their licence notices |
| 2026-11-01 | patch | `en-variants.txt`: `judgment`/`judgement` (and `-al`) right in every English, now that each English checks only its own SCOWL dictionary (craft-libs#483 review) |
| 2026-11-01 | minor | Milestone 4 phase 3 follow-up: ar, be, ca, ca-valencia, cy, el, fr, he, hy, id, is, ka, lv, mk, sk, sr, sr-latn, tr, af, ko, gl unhidden (61 published, 16 hidden, 3 Excluded); hidden reasons name the current compounding gaps; gl check word fixed (gl now published); Moby `en-us.dic` loses typos teh, abd, nad, ur, u, r; licence file names checked as the consumer does; craft-proof dev-dependency pinned at storytold/craft-libs `8f16fe5a` (the restacked #485 stack top), still valid once #482–#490 merge with merge commits |
| 2026-11-02 | minor | Milestone 4 phase 3, compounding: da, de, de-at, de-ch, eo, et, fa, hr, hu, nb, nl, nn, sv, sv-fi unhidden (75 published, 2 hidden, 3 Excluded; mn stays hidden: suggestions take 200–580 ms over its 12,066 affix rules; th stays hidden: craft-proof splits words only at spaces and punctuation, so a Thai sentence would be checked as one word); needs craft-proof flag-based compounding (craft-libs #497, merged at `47c9f625`); de, de-at, de-ch check words now capitalised nouns and et, nb, nn have real misspellings, each checked with the hunspell 1.7.5 CLI; nb and nn hyphenate with hyph-no.tex (their own files hold only exceptions) |
| 2026-11-03 | minor | 3a: PdfCraft cutover sync (`pdfcraft@ecdd1e6`, 12 new PdfCraft locales); PdfCraft wired now, not last: it already draws logical catalogs on storytold/egui |
| 2026-11-03 | patch | Checker takes each locale's plural form count from the runtime crate (`craft-i18n` at storytold/craft-libs `32b2fd8a`, a normal dependency; its own CLDR 48.2.1 data on `icu_plurals` since craft-libs#210), replacing `intl_pluralrules` 7.0.2 and its Hebrew workaround; `icu_plurals`' compiled data has no `ckb` rules (1 form), the crate's data has 2; `tests/runtime_agreement.rs` checks every locale's count and that the crate accepts every catalog's `@plural` rows; no existing locale's count changed |
| 2026-11-03 | minor | File sizes (storytold/craft-libs#204): `@plural {n} byte\|{n} bytes` moved from 20 locales' `pdfcraft.tsv` to `common.tsv` (additive, ADR 0003); `unit` rows (`{n} KB`, `{n} MiB`, …) only where the symbol differs from English: `fr`, `ru`, `uk` |
| 2026-11-05 | minor | CADCraft resync before its cutover (`cadcraft@b23d362` → `cadcraft@c4d9e6a`): its Greek, Hungarian, Turkish and Vietnamese catalogs (added in the app after the 3b migration) are new here; every other CADCraft catalog takes the app's added and corrected rows and drops the prompt texts the app reworded |
| 2026-11-08 | minor | 3b: PhotoCraft cutover sync (`photocraft@8d2e615` → `photocraft@70c0911`, 12 new PhotoCraft locales: ar, az, da, fi, he, hi, nb, ro, sv, ta, th, ur; `az` is new to craft-i18n); Hebrew's 35 `@plural` rows repeat the `other` form for CLDR's third form; credits follow PhotoCraft's `LICENSE-translations.txt` |
