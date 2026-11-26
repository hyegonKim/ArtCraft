# Language data format

How the Language data other than English is described, checked and published as **Language
packs**: one downloadable archive per locale, with an index the apps pin. English (and the
interim carry-over listed in [`language-data/README.md`](../language-data/README.md)) is
committed and embedded; everything here is fetched from its upstream at a pinned commit and never
committed. Licence classes are [ADR 0004](adr/0004-third-party-language-data.md); why the data
lives here is [ADR 0005](adr/0005-language-data-lives-here.md). The audit the manifest was seeded
from is dated 2026-10-31.

## The manifest: `language-data/manifest.toml`

One `[[file]]` per upstream file:

```toml
[[file]]
locale = "nl"                 # lowercase BCP 47
kind = "spelling-aff"         # spelling-aff | spelling-dic | hyphenation | licence
url = "https://raw.githubusercontent.com/LibreOffice/dictionaries/<commit>/nl_NL/nl_NL.aff"
commit = "<upstream commit sha>"
sha256 = "<hex>"
size = 12345                  # bytes
licence = "BSD-3-Clause"      # SPDX expression
class = "Embeddable"          # Embeddable | Download-only | Excluded
name = "Nederlands"           # optional, first entry per locale wins: endonym
note = "…"                    # optional: the licence statements, quoted, when evidence is weak
```

- **A pack is every entry of one locale.** `spelling-aff`/`spelling-dic` are a Hunspell pair,
  `hyphenation` a TeX pattern file (an unmodified `hyph-*.tex` from tex-hyphen, compiled when
  loaded), `licence` a licence or README text that travels with the data.
- **`licence`** is the licence the file is used under. When upstream offers several, the one in
  the least restricted class is recorded (ADR 0004). When the evidence is weak or contradictory,
  `licence` lists every licence the statements offer, `class` is Download-only (or Excluded, if
  any statement is non-commercial or no-derivatives) and **`note`** quotes the statements and
  where they are (bg, hr, pt-pt, ta, uk, eu, ug). A `licence` entry carries the licence and class
  of the data it describes.
- **`class`** may be more restricted than the licence allows, never less; when it is, the data
  entries need a `note`.
- **Licence texts.** Every locale has at least one `licence` entry, the upstream notice (a
  hyphenation file's own header counts: the `.tex` is listed again as a `licence` entry when its
  licence has no SPDX id). On top of that, every licence id its data files name must have a full
  text in the pack: an SPDX id's standard text is `language-data/licences/<id>.txt` (from
  [spdx/license-list-data](https://github.com/spdx/license-list-data) at `31846481`, committed
  here), added to the archive as `LICENCES/<id>.txt`; a `LicenseRef-` id's text is a `licence`
  entry whose `licence` names it.
- **Excluded** entries are recorded (so the decision and its evidence stay visible) and never
  published.
- Sources are pinned raw files: LibreOffice/dictionaries, tex-hyphen, and an upstream repository
  or wooorm/dictionaries where the audit names it the best source.

### Licence allow-list

Every SPDX id in an expression must be in the checker's allow-list
(`crates/craft-i18n-tests/src/packs.rs`, `LICENCES`), which gives each its class. `OR` takes
the less restricted side and `AND` the more restricted. A licence with no SPDX id gets a
`LicenseRef-` id there first (today `LicenseRef-BSD-Lebedev`, the Russian dictionary's BSD
licence with a "modified versions must be marked" clause, and `LicenseRef-hyph-bg`, the
Bulgarian patterns' permissive licence). `WITH` exceptions and `+` are not allowed: write
`-or-later`.

## Check words: `language-data/checks/<locale>.toml`

```toml
correct = ["huis", "boek", "school", "water", "stad"]   # at least 5
incorrect = ["boekk", "schooll", "werkken"]             # at least 3
hidden = true                                          # optional
reason = "craft-proof doesn't support compounding yet (COMPOUNDFLAG)"
```

Common words, written here (this repository's licence). A regional locale without its own file
uses its language's (`es-mx` uses `es`). **`hidden = true`** keeps a pack out of releases while
`craft-proof` can't read it, or can't yet use it well; it needs a `reason`. The reason names the
gap in the pinned `craft-proof`: an unsupported feature (`craft-proof doesn't support <feature> yet
(<AFF flag>)`), missing word segmentation, or suggestions too slow to use. It is a statement of fact
about the pinned `craft-proof`, so refresh it when the pin moves.
As of 2026-11-02 (craft-proof at storytold/craft-libs rev `47c9f625`, the merge of #497, flag-based
compounding): 80 locales, 3 Excluded (eu, ug, uk), 2 hidden (mn, whose suggestions take 200–580 ms over
its 12,066 affix rules; th, which needs word segmentation), 75 published.

## Checks

Every CI run (`cargo test --workspace`, no network):

- the manifest parses; every entry has a valid lowercase BCP 47 locale (no English), a URL that
  parses as `https://raw.githubusercontent.com/<owner>/<repo>/<commit>/<path>` with its 40-digit
  `commit` as the third segment (no empty or dot segments, query or fragment), a 64-digit
  lowercase `sha256`, a size of 1 byte to 64 MiB (`craft-proof`'s limit), a licence and a class;
- every licence parses as SPDX, uses only allow-listed ids, and its class is not less restricted
  than the licence allows; a more restricted class has a `note`;
- no duplicate (locale, kind); licence file names are `[A-Za-z0-9._-]`, not starting with `.`,
  unique in a locale and never clash with a standard text's `<id>.txt`;
- a locale with spelling has both `.aff` and `.dic`; every locale has a name and a licence
  entry, and every data licence id has a full text (above); every file in
  `language-data/licences/` is an allow-listed SPDX id;
- every locale with spelling that isn't Excluded has check words (or is hidden with a reason),
  none empty, and every checks file names a manifest locale;
- no locale with an Excluded entry is published.

With `CRAFT_I18N_FETCH=1` (the weekly and release workflow,
[`.github/workflows/language-packs.yml`](../.github/workflows/language-packs.yml)):

- every file downloads and matches its `size` and `sha256` (`every_file_matches_its_pin`);
- every published pack's `.aff`/`.dic` loads through `craft-proof`'s affix engine, accepts every
  `correct` word and rejects every `incorrect` one, and its hyphenation patterns compile
  (`packs_load_and_check`). Hidden packs are tried too and reported, so the log says when one
  could be published.

Files are cached by sha256 in `$CRAFT_I18N_CACHE` (default `target/language-data-cache`).

## Release assets

A craft-i18n GitHub release `language-packs-<n>` (pushed tag, or the workflow run by hand with a
release name) holds the assets below. A release is never modified: the workflow refuses a name
that already exists, creates the release as a draft, uploads every asset, then publishes it. A
fix is a new `language-packs-<n+1>`. A manual run must be on `main` and refuses a tag that
already exists; runs for the same release name never overlap. If an upload fails, the run
leaves a **draft** release behind: delete it before running again, or the rerun refuses
the name.

- **`<locale>.tar.zst`** per published pack (no Excluded entry, not hidden, checks passed). At
  the archive root: `spelling.aff`, `spelling.dic`, `hyphenation.pat` (any may be absent),
  `LICENCES/<original file name>` for each `licence` entry, `LICENCES/<SPDX id>.txt` for each
  standard text, and `pack.toml` (locale, name, licence, class, version, `upstream` (as `source`
  below), `licence_texts`, and a `[[source]]` per file with its url, commit, sha256, licence and
  class). Deterministic: sorted
  names, zero times and owners, mode 0644, zstd level 19.
- **`index.json`**:
  ```json
  { "format": 1, "release": "language-packs-1",
    "packs": [ { "locale": "nl", "name": "Nederlands", "version": "<craft-i18n commit>",
                 "url": "https://github.com/storytold/craft-i18n/releases/download/language-packs-1/nl.tar.zst",
                 "sha256": "<hex of archive>", "size": 123456,
                 "licence": "BSD-3-Clause", "class": "Embeddable",
                 "spelling": true, "hyphenation": true,
                 "source": "https://github.com/LibreOffice/dictionaries/tree/<commit>/nl_NL" } ] }
  ```
  `source` (optional, additive to format 1) is the upstream folder of the pack's first data file
  at its pinned commit, for the app to show before install. A pack's `licence` is its files' distinct licences joined with `AND`; its `class` its most
  restricted file's.

Apps pin the index by URL + sha256; packs have no update checks of their own.

## The `language-packs` tool

```sh
cargo run --release --bin language-packs -- check     # the offline checks
cargo run --release --bin language-packs -- fetch     # download + verify every file into the cache
cargo run --release --bin language-packs -- build --out DIR --release NAME --version SHA \
    [--base-url URL] [--only nl,fr,pl] [--cache DIR] [--no-download]
```

`--base-url` defaults to the release's download URL; `--only` builds just those locales (and
fails on an unknown, hidden or Excluded one).

### Testing an index locally

To try an app against packs before a release is published, build them with archive URLs on a
local server and serve the folder:

```sh
cargo run --release --bin language-packs -- build --out /tmp/packs \
    --release language-packs-test --version "$(git rev-parse HEAD)" \
    --base-url http://127.0.0.1:8765/ --only pl,ru,it
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/packs
```

Then point the app's pack index at `http://127.0.0.1:8765/index.json` with the sha256 of that
file (`sha256sum /tmp/packs/index.json`). Leave out `--only` to build every published pack.
