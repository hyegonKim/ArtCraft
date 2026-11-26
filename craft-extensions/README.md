# craft-extensions

One place to find, install and switch **themes, plug-ins and resources** for the Craft App Suite,
shared by every app and reachable from each of them. The browser itself is native to ArtCraft Launcher;
this repository holds the shared contract and the on-disk store the launcher and the apps both use.

**Status: provisional and early.** No app or launcher ships it yet. The API will change.

The architecture for the full plugin and add-on system has been reviewed and its six open decisions are
settled: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), with its [decision record](docs/DESIGN-DECISIONS.md),
the [topic proposals](docs/TOPIC-PROPOSALS.md) behind it and a [gap analysis](docs/ARCHITECTURE-GAP-ANALYSIS.md)
whose maintainer answers the direction below and [`docs/THREAT-MODEL.md`](docs/THREAT-MODEL.md) now follow.

## Direction

- **Internal first, open store next.** Release builds install extensions built by the Craft organisation,
  plus files a person adds themselves. The open community store is the accepted target: it opens when the
  catalogue client lands, with the review gates in [`docs/THREAT-MODEL.md`](docs/THREAT-MODEL.md) and an
  outside security review before public launch.
- **Native plug-ins are accepted, labelled and consented.** The design lists native plug-ins, out of
  process, labelled "Native: full access to your computer", with explicit consent and two-maintainer review
  of every native listing and release. Nothing native is built here yet.
- **The store builds and hosts nothing.** Assets download from the author's GitHub Releases, with a public
  source link and a pinned digest.
- **Install once, enable per app.** Apps read the shared store; a running app notices an install without a
  restart. An app with no launcher can still install from a file.
- **Ranking without telemetry.** The store ranks by GitHub stars fetched centrally, no download telemetry.
  Pilots run in order EffectCraft, FigCraft, PhotoCraft.

## Crates

| Crate | What it is | Builds for wasm |
|---|---|---|
| `craft-ext-core` | Package manifest, ids, paths, trust tiers, the theme palette and contrast check, the catalogue index schema. Pure data and validation. 63 tests; 27 deliberate breakages each caught (`docs/mutation_check.py core`). | yes |
| `craft-ext-store` | The shared store on disk: bounded unpacking, atomic installs, enable per app, the read side apps use. Never touches the network. 48 tests; 34 deliberate breakages, 31 caught and 3 equivalent with stated reasons (`docs/mutation_check.py store`). | no |

## Rules this code follows

- Everything read from a package, an index or the network is hostile input. Sizes are capped before
  parsing, ids and paths are checked by character class, archives are unpacked with count, size and ratio
  limits and no path may leave its folder.
- Non-test code never panics (`unwrap`, `expect`, `panic!` are denied in every crate root). No `unsafe`.
- A package never states its own trust level; the host assigns it from where the package came from.
- Themes carry no code. The code on `main` gives plug-ins no files, network or clock and requires the
  permissions list to be empty; the accepted design replaces that with permission families — declared in
  the manifest, granted by the person in the host, checked on every call — once the operations kit lands.

## What is not done

Signatures, the signed expiring catalogue, the revocation list wired to a running launcher, the permission
families, the native plug-in host, the sandbox runtime host, the catalogue client and the manager interface
are not built, nor are fuzzing and the outside security review. Nothing has been run against a live server,
on macOS or Linux, or with a real OS-level file lock under heavy contention. The state-file rollback after
a failed save is written but not exercised by a test.

## Develop

```sh
cargo fmt --all --check
cargo clippy -p craft-ext-core --all-targets -- -D warnings
cargo test -p craft-ext-core
cargo test -p craft-ext-store
python docs/mutation_check.py core   # or store; run alone, it edits sources
cargo check -p craft-ext-core --target wasm32-unknown-unknown
```

Run one crate at a time. Licensed under MIT OR Apache-2.0.
