# Architecture gap analysis

Status: **for review, 2026-11-10.** This compares the proposed [plugin architecture](ARCHITECTURE.md) with this repository at `main` `a6c5fae` and with the unmerged `wip/extensions-handover` branch (`ffd8512`). It comes from reading every source file, the [threat model](THREAT-MODEL.md), the README and the handover notes on that branch. No code was built or run for it.

**In short:** the code on `main` is a careful first slice. It covers the package envelope, bounded unpacking, host-assigned tiers, the theme palette and an atomic shared store. Most of it carries straight into the design. Four store behaviours have to change before any app relies on them, and the manifest and index need a second schema. Everything for executable plugins is still to build: operations, permissions, the runtime host, contracts, the SDK, the catalogue client and the manager UI. The design also **disagrees with six positions this repository has already written down**. Those are maintainer decisions, listed in section 4, and nothing else should change until they are settled.

## 1. What exists on `main`

| Module | What it does today | Verdict |
|---|---|---|
| `ext-core/id.rs` | Extension and app IDs from a small lowercase alphabet, at least two segments, first-party prefixes reserved | Keep. One change: the length cap (section 3) |
| `ext-core/path.rs`, `text.rs` | Package paths with no escape, device names or case collisions; display text without bidi, invisible or control characters | Keep as is |
| `ext-core/version.rs` | Strict `major.minor.patch` | Keep. It also works for contract versions |
| `ext-core/manifest.rs` | `craft-extension.json` schema 1: one kind, files with SHA-256, per-app `compat {min_app, interface}`, optional provenance, permissions that must be empty, a 64 KiB cap, schema read first | Keep the envelope. Schema 2 is needed (section 3) |
| `ext-core/palette.rs` | Shared theme palette, per-app overrides, WCAG 4.5:1 contrast and the dark flag | Keep. Theme scope is to be revisited later anyway |
| `ext-core/tier.rs` | Tiers `FirstParty`, `FromFile`, `ReviewedThirdParty`, assigned by the host; third party refused unless the `third-party` feature is on | Keep the mechanism. Its policy is a maintainer decision (section 4) |
| `ext-core/index.rs` | Catalogue index schema 1: entries with URL, SHA-256, size and apps; version and expiry; revocation by `id` and `below`; unsigned | Superseded by the signed catalogue (section 3) |
| `ext-store/location.rs` | One shared folder under local data, `CRAFT_EXT_DIR` override, the same in portable mode | Keep. It is the design's shared store folder |
| `ext-store/unpack.rs` | Four-pass bounded unpack: entry count, links, encryption, manifest-archive agreement, streaming size, ratio and hash checks | Keep as is. It answers threats T1–T3 |
| `ext-store/state.rs` | `state.json`: installed list, enabled set per app, active theme per app, a `generation` counter, newer schema refused | Extend (section 3) |
| `ext-store/store.rs` | Install from zip or file, uninstall, enable, choose theme; staging, a cross-process lock and an atomic write through `craft-atomic` | Keep the structure. Change four behaviours (section 3) |
| `ext-store/reader.rs` | The read side apps use: installed and enabled for an app, active theme re-checked from disk, bad data treated as absent | Extend (section 3) |
| `docs/THREAT-MODEL.md`, `docs/mutation_check.py` | 20 threats, 13 gates; a mutation harness over the safety rules | Keep the harness. The threat model needs updating once section 4 is decided |
| CI | fmt, clippy, wasm check of the core crate, tests on Linux and Windows | Keep |

**Already matching the design:**
- every byte treated as hostile;
- no panics and no `unsafe`;
- the core crate builds for wasm;
- tiers never read from packages;
- reserved first-party prefixes;
- a bounded, exact unpack;
- one atomic state file with a generation counter;
- readers that need no launcher and never write;
- a newer schema refused rather than half-read;
- contrast enforced;
- plain-language errors;
- a mutation check.

**On the handover branch:** an optional plugin `thumbnail.png` (512 KiB cap, validated with the `image` crate), handover notes, a generated plan page and three sample themes. The thumbnail fits the design's listing screenshots. It should come in through manifest schema 2 rather than as a schema 1 change.

## 2. What the design needs that does not exist

| Piece (architecture section) | State | Delivery unit |
|---|---|---|
| `craft-ext-ops`: `Invocation`, principals, operation descriptors, `authorize`, errors, jobs, exit codes (7) | Missing | P2 |
| Permission families, grant records with a generation, consent records (8) | Missing; permissions must be empty | P1, P2 |
| `craft-ext-host`: shared wasmi profile, budgets, instance rules, failure contract, native helper protocol (9) | Missing | P3, P10 |
| `craft-ext-conformance`: contract suites, hostile-module suite, benchmarks, reference host (9, 19) | Missing | P3 |
| `craft-ext-contracts` and the inventory generator; the first contract `raster.filter@1` (6) | Missing | P4 |
| Author SDK: `craft-guest`, `craft-guest-<contract>`, macros, `Craft.toml` (20) | Missing | P4 |
| `craft-ext` CLI: create, check, test, dev, pack, prepare-release, verify (20) | Missing | P4 |
| Manager UI component and `plugin` CLI commands for apps and the launcher (14) | Missing here; the launcher has an Extensions screen on an unmerged branch | P5 |
| Catalogue client: TUF verification, download with a pinned digest, revocation sync (12, 13) | Missing. The README says the store never touches the network | P7 |
| Leases, data dependencies, large-file references, private storage, setup export and import (10, 11, 14, 17) | Missing | P1, P5 |

## 3. What has to change in the existing code

Each row is a separate small pull request with a test that fails before the change, as the house rules require.

| # | Today (file:line on `main`) | Design | Why it matters |
|---|---|---|---|
| C1 | Install enables the package for every app its manifest lists (`store.rs:201`; the tests assert it, `store/tests.rs:52`, `:117`) | Install enables nothing; each app is set up explicitly | Otherwise a multi-app plugin runs in apps the user never chose, before any permissions are granted |
| C2 | Updating deletes the previous version's folder right after the state write (`store.rs:213`) | Keep a release while any lease uses it; delete it in cleanup | An export or app still reading the old files breaks mid-run |
| C3 | A newer version is required (`Downgrade`, `store.rs:184`), and the same version is refused (`AlreadyInstalled`, `:187`) | A version change the user chooses, older ones included; the same digest is idempotent | The design lets users pick any listed version |
| C4 | Only `FirstParty` is protected from replacement (`store.rs:181`) | A file never replaces a store-installed ID; the user removes the store copy first | Same-ID takeover of a reviewed plugin by a local file |
| C5 | `Installed` keeps ID, version, kind, name, tier and apps only (`state.rs:23`) | Also the archive digest, the retained verified manifest, the source (listing or file), and the label | Without them the app cannot check bytes at load, detect a swapped file or apply revocation by digest |
| C6 | The reader returns a plugin folder if it exists (`reader.rs:65`); it never re-hashes, checks revocation or applies policy | Load-time check of digest, compatibility, revocation and grants through one eligibility function | The threat model's G6 asks for re-validation on every load; this is where it goes |
| C7 | One `schema` number; newer is refused (`state.rs:93`); unknown fields are dropped on the next save (no `deny_unknown_fields`, so an older writer erases newer fields) | `write_version` and `read_version`, as SQLite's header has: older apps keep reading; an app refuses to write a newer state | Mixed app versions are normal across sixteen apps |
| C8 | Release folders are `packages/<id>/<version>/` (`store.rs` `package_dir`) | `packages/<id>/<version>-<digest>/` | Two different files with the same version must not share a folder |
| C9 | Only enable and theme are stored per app | Per-app setup: contributions, grants, settings, pin, time raises | All of section 8 depends on this record |
| C10 | `ExtensionId` is capped at 64 characters (`id.rs:31`) | `io.github.<owner>.<name>`, verified against the GitHub owner | GitHub owners can have 39 characters and repository names 100, so verified IDs can exceed 64. Raise the cap (for example to 128) or define how a name is shortened |
| C11 | Manifest schema 1: one kind, one `plugin.wasm`, `compat.interface` as one number, permissions empty | Schema 2: contributions per app, contracts with major and minimum minor, interaction style, permission families, native artifacts per platform, data dependencies, large-file references, thumbnail and screenshots | Schema 1 cannot describe a multi-contribution or native package. Keep schema 1 readable with its current meaning |
| C12 | Index schema 1 is unsigned; an index with the *same* version as one already seen is accepted (`index.rs:126`, `<` rather than a content check); revocation is `id` + `below` only; the entry's `tier` is data from the index | A TUF-signed catalogue; same version with different content refused; revocation by digest, version range or publisher, with severities; labels assigned by the client | Replay and swap defences; matches threat-model gates G2–G4 |
| C13 | The store never consults revocations (`is_revoked` is not called from `ext-store`) | Revocations applied at enable, load and update, including for file installs with a matching digest | Gate G4 |
| C14 | `Cargo.toml` `repository` still points at `Jeremy8776/craft-extensions` | `storytold/craft-extensions` | Already fixed on the unmerged `chore/repo-url-storytold` branch |

A design question for C13 and section 2: the README promises "`craft-ext-store` never touches the network". The design keeps that by putting the catalogue client in its own crate (for example `craft-ext-catalog`). The store then stays offline, and the launcher's download routine moves there.

## 4. Where the design disagrees with decisions already recorded here

These are written in the README, the threat model or the handover decisions (d3, d11 and others). The new design changes them. They needed an explicit maintainer decision before code follows the design; the answers are below. Until the threat model is updated, its gates stay in force.

| # | Recorded here | New design | What has to be decided |
|---|---|---|---|
| D1 | Third party "designed in and switched off" behind the `third-party` feature until gates G1–G13 pass, including a two-maintainer review rule (G10), an independent outside review (G12) and an explicit decision (G13) | An open community store: a bot plus a quick human look for risky cases, bot auto-merge for routine updates with a 48-hour hold, an internal security review only, and no responsibility taken for listed plugins | Whether to open third party on these terms, and which gates are replaced, kept or dropped. The cargo-feature guard (G5) can stay as the switch |
| D2 | Version 1 plug-ins get "no files, network or clock", have empty permissions and import nothing (T8, G6) | Session plugins call every operation the app's CLI offers (except management, core and internal), under shared permission families with consent | The permission model replaces G6's "empty permissions" |
| D3 | No native code; the sandbox is the only plug-in runtime | Native plugins allowed in the store too, out of process, labelled "full access to your computer", with no OS confinement required | Accepting native code that no sandbox contains |
| D4 | Provenance: "decide whether the hub builds plug-ins from source" (T15, G9) | The store builds nothing and hosts nothing; files come from authors' GitHub Releases with a pinned digest | Settles G9 as "no store builds" |
| D5 | Ranking by downloads and other signals (handover d11) | GitHub stars fetched centrally, no download telemetry; official plugins first within a relevance tier | Ranking signal |
| D6 | PhotoCraft plug-in host first (handover task a-photo-1); existing app ABIs kept | EffectCraft, then FigCraft, then PhotoCraft; every app's current plugin system removed and replaced | Pilot order and removal of existing systems. The PhotoCraft theme adapter on its own branch fits the design's L0 level unchanged |

**Answered in review (2026-11-10).** All six were answered on this pull request and accepted:

| # | Answer |
|---|---|
| D1 | The open store is the target. The `third-party` feature stays off until P7. An independent outside security review (G12) stays a gate for public launch (P12). The two-maintainer rule (G10) stays, for native listings and releases only |
| D2 | Permission families accepted. G6 becomes "declared, host-granted, generation-checked" instead of "empty". Pilots ship transform plugins only until P2 lands |
| D3 | Native accepted, with a human look (two maintainers) on every native release and never a bot auto-merge. OS confinement is a P10 goal, not a listing requirement |
| D4 | No store builds. Every listing needs a source link and a pinned asset digest |
| D5 | GitHub stars fetched centrally, no download telemetry |
| D6 | EffectCraft, then FigCraft, then PhotoCraft. Each app's old plugin system is removed in the same change set as its new adapter |

The next step is a docs pull request that updates `THREAT-MODEL.md` and the README to these answers.

Consistent with what is recorded here: tiers never self-declared (the design's labels map `FirstParty` → Official, `ReviewedThirdParty` → store listing, `FromFile` → Not reviewed); no silent permission or key change (G7, and the design's "always ask" updates); reserved first-party prefixes (G1); the shared folder (d5); the standalone public workspace (d7); and "plug-ins should support a vast range of ability" (d12).

## 5. Suggested order of work in this repository

1. **Update `THREAT-MODEL.md` and the README** to the section 4 answers in one docs pull request.
2. **C1, then C2, then C4**. Each is a small store change with a failing-first test. C1 and C2 make the store safe for apps to use, whatever is decided in section 4.
3. **C5, C6, C8 and C13** together as the "verified installed release" change, plus C3.
4. **C7** state versioning, before any of the fields above ship to users.
5. **C10 and C11**: manifest schema 2, starting with a short schema document for review.
6. **New crates** in delivery order: operations (P2), host and conformance (P3), contracts and SDK (P4), manager component (P5), catalogue client (P7, C12).

The mutation harness should gain one mutant per new rule (for example "install enables apps", "release deleted while leased").

## 6. Limits of this analysis

- **Sources:** `main` `a6c5fae` and `wip/extensions-handover` `ffd8512`, read on 2026-11-10. The launcher and PhotoCraft branches mentioned in the handover notes were not re-read for this document.
- **Nothing was run:** no tests, mutation harness or CI. All claims come from reading the source.
- **GitHub name limits** (39-character owners, 100-character repositories) are as GitHub documents them; recheck them before changing `id.rs`.
