---
status: accepted
---

# Language data lives here, pinned to upstream; English is embedded, the rest is downloaded

Spelling dictionaries and hyphenation patterns move into this repository as data, next to the
catalogs; the code that reads them stays in craft-libs (craft-proof, craft-text), as
[ADR 0002](0002-runtime-in-craft-libs-data-in-craft-i18n.md) already does for translations. Apps
each carried their own copy (Moby in WordCraft and DesignCraft, SCOWL in PhotoCraft, `hypher`
features in DesignCraft and craft-text) or none, and about 25 open issues ask for a missing
language.

- Data sits in `language-data/<locale>/` (lowercase BCP 47, regional falling back to its
  language). Upstream files are published with their contents unchanged (`.aff`/`.dic`, TeX
  patterns), under fixed names (`spelling.aff`, `spelling.dic`, `hyphenation.pat`); compiling
  them is the runtime's job. WordCraft's Moby list (`en-us/en-us.dic`) stays as the hyphenation
  word list, with our own word lists under their current names, until craft-proof no longer
  needs them. The suite word list is `language-data/words.txt`, with optional
  `<locale>/words.txt`; it is written here, committed and embedded with English, and a locale's
  list goes in its Language pack.
- English (en-US, en-GB, en-CA and en-AU, from SCOWL) is committed and embedded in every build that
  has `CRAFT_I18N_DIR`; craft-proof may check a regional English with another's list (it checks
  en-AU as British-spelling English). A build without it (a fork, docs.rs, offline) still builds,
  with no embedded Language data: spelling falls back to the system's Hunspell dictionaries,
  hyphenation is off. Other languages are a **manifest** pinning each file's upstream URL, commit,
  sha256 and licence; CI fetches, checks and publishes them as release assets, so cloning stays
  small and English-only builds stay offline.
- Other languages are **Language packs**, one per locale (spelling dictionary, hyphenation
  patterns, licences), downloaded through the suite's font store, its sha256 checks and its single
  consent setting (storytold/craft-libs#223), only after the user opts in. A **Language build**
  embeds the Embeddable packs it names instead. Until downloads ship, a missing language falls
  back to the system's Hunspell dictionaries, and data an app embeds today stays embedded; it
  leaves the build in the release that ships downloads.
- Users pick packs in the app's language options, or from a non-blocking prompt when a document
  has text in a language with no pack. Only packs that pass CI's load-and-check test are offered.
- A spelling dictionary is looked up in the user's chosen dictionaries folder, then an installed
  pack, then the system's Hunspell dictionaries. The pack directory is one more search path
  for craft-proof's dictionary search (craft-libs#31), not a separate mechanism.
- The web build is English-only until the font store works there.
- Grammar rules, suggestion tuning and other proofing code are not Language data: they live in
  craft-proof for every locale and work without a pack.
- What an app downloads is the manifest of its Pin: new data arrives when the app moves its pin.
- ICU4X word segmentation data stays a crate dependency of craft-text; it is versioned with its
  code.

## Depends on

Accepted here means where the data lives and how it is pinned. Downloads wait on
storytold/craft-libs#223 (decisions 1, network access, and 4, index hosting), and reading the data
from here needs craft-text (craft-libs#19) and craft-proof (craft-libs#31, open) to agree. Until
then apps keep what they embed today and fall back to system dictionaries.

## Consequences

- Cloning stays small: only English is committed; CI fetches and checks the rest.
- Every app's spelling and hyphenation come from one pinned source, so a fix reaches all apps on
  their next pin move; moving apps from Moby or SCOWL 50 to the shared English changes which words
  are flagged.
- The web build has no language beyond English until the font store works there.

## Considered options

- **Commit every language**: rejected, 100–200 MB cloned by every app build.
- **Git LFS**: rejected, an extra tool for every contributor and mirror.
- **Keep data in craft-libs crates**: rejected, it is what scattered the copies, and cargo
  features can't download a language after install.
- **A second download mechanism**: rejected, two consent switches for one suite.
