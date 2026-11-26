---
status: accepted
signed-off: Martin Ball (benandez), craft-i18n maintainer, 2026-11-01
---

# Third-party Language data is allowed; translations stay clean room

Spelling dictionaries and hyphenation patterns can't be written from the plain meaning of the
English: they are decades of upstream work (Hunspell dictionaries, TeX hyphenation patterns). They
are taken from open-licensed upstream projects, unchanged, pinned and recorded in
`ATTRIBUTION.md`. The clean-room rule keeps applying to translations of the interface, which are
never copied. The suite word list is written here and is under the repository's own licence.

Every upstream file has a **Licence class**, recorded in the manifest next to its licence. The
class decides where the file may go:

| Licence class | Licences | Embedded in a binary | In a Language pack |
|---|---|---|---|
| **Embeddable** | Public domain, CC0, MIT, BSD, Apache-2.0, Unicode-3.0, SCOWL's own licence (`LicenseRef-SCOWL`), CC-BY 4.0; MPL and LPPL when unmodified | yes | yes |
| **Download-only** | LGPL, GPL, AGPL, CC-BY-SA; weak or contradictory licence evidence the author hasn't resolved | no | yes, as a separate unmodified file |
| **Excluded** | no licence; any statement that is non-commercial or no-derivatives | no | no |

- Files are never modified. LPPL patterns are shipped as the unmodified `.tex` and compiled when
  loaded, so no "Compiled Work" is distributed. Our own corrections go in our own files (for
  example `en-extra.txt`), under this repository's licence.
- When a file is offered under several licences, the one in the most permissive class above is
  used and recorded (a file tri-licensed MPL/LGPL/GPL is Embeddable, under MPL).
- A file whose licence evidence is weak or contradictory (say a README saying GPL and an `.aff`
  header saying LGPL) is **Download-only** until its author confirms otherwise, unless one of the
  statements is non-commercial or no-derivatives, which makes it **Excluded**.
- CI holds the table as an SPDX allow-list (`crates/craft-i18n-tests/src/packs.rs`), so a licence
  with no SPDX id gets a `LicenseRef-` entry before it qualifies. Packs are described by
  `language-data/manifest.toml` ([format](../language-data-format.md)); committed files keep their
  licence and class in `ATTRIBUTION.md`'s Language data table.
- The licence name and source link show in the app's language list before install, and the full
  licence text of everything embedded or installed shows in About › Language data. No separate
  licence prompt.
- CI rejects a file with no recorded licence or class. **Excluded entries are recorded** in the
  manifest, with the evidence, so the decision stays visible, **and never published**; no
  committed file may be anything but Embeddable. A Language build naming a pack that isn't
  Embeddable is rejected once Language builds exist.

**Signed off** by Martin Ball (GitHub benandez), craft-i18n maintainer, on 2026-11-01: the
table above holds, and the switch that failed every LGPL, MPL, LPPL and GPL file until then is
retired. It is not legal advice. The one exception to "no language beyond
English" is the carry-over: the permissive (MIT, BSD, public-domain) data WordCraft embedded
before the move (Moby English, de/pl/pt/it hyphenation, Russian extra words) stays embedded until
the release that ships downloads ([ADR 0005](0005-language-data-lives-here.md)).
The audit behind it (2026-10-31, 66 spelling and 88 hyphenation rows, each with the
licence quoted) is the starting manifest.

## Consequences

- The repository is no longer one licence: files under `language-data/` keep their upstream
  licences, next to them as `LICENCE-<source>.txt` (until the manifest lands, the notice in each
  file's header and its `ATTRIBUTION.md` row). README and `ATTRIBUTION.md` say so.
- Languages beyond English ship as Language packs (downloads), apart from the carry-over.
- Every new dictionary needs its licence quoted and classed in the manifest; CI does the rest.

## Considered options

- **Write our own dictionaries**: rejected, impossible at the quality users expect.
- **Permissive licences only**: rejected, it leaves out most Hunspell dictionaries (many are
  LGPL or MPL) and the languages users ask for most.
- **Embed LGPL with a notice**: rejected, LGPL lets users replace the file, which is awkward when
  it is compiled into a native or wasm binary and trivial when it is a separate file.
