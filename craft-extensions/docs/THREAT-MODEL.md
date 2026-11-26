# Threat model and security gates

Status: aligned with the six maintainer decisions settled in the [architecture gap analysis](ARCHITECTURE-GAP-ANALYSIS.md) (2026-11-10). The threat list is unchanged; several gate answers changed. Until the new pieces are built, the code on `main` still enforces the version-1 rules.

## Settled decisions

The [gap analysis](ARCHITECTURE-GAP-ANALYSIS.md) asked six questions and the maintainers answered all six. This document follows those answers:

- **D1 — Open store.** The open community store is the target. The `third-party` feature stays off until the catalogue client (`P7`) lands; an outside security review (`G12`) stays a gate for public launch; the two-maintainer rule (`G10`) now covers native listings and native releases only.
- **D2 — Permission families.** `G6` changes from "permissions empty" to declared, host-granted and generation-checked. Pilots ship transform plug-ins only until the operations kit (`P2`) lands.
- **D3 — Native plug-ins.** Accepted, out of process, labelled and consented (section below).
- **D4 — Provenance.** The store never builds or hosts plug-in binaries (`G9`, settled).
- **D5 — Ranking.** GitHub stars fetched centrally, no download telemetry.
- **D6 — Pilot order.** EffectCraft, then FigCraft, then PhotoCraft.

## What is protected

- A person's files and machine (an extension must not read, write or run anything it was not given).
- The integrity of the apps (a bad extension must not crash an app or corrupt a document).
- The integrity of the catalogue (nobody can pass off their package as ArtCraft's, or roll a person back to a vulnerable version).

## Kinds and what each can do

| Kind | Content | Capability in version 1 |
|---|---|---|
| Theme | JSON palette and per-app colour overrides | None. Data only. |
| Plug-in | One WebAssembly module with a JSON manifest | Pure computation in a sandbox: no host imports, so no files, network, clock or randomness. Memory, instruction and time limits. |
| Resource | Files in formats an app already imports (swatches, LUTs, brush sets) | None of its own. The app's existing importer reads it, so that importer is the attack surface. |

## Tiers

| Tier | Who | Release builds |
|---|---|---|
| First party | Built and released by the Craft organisation | Accepted |
| From a file | The person's own file | Accepted, with a plain warning; the same validators and sandbox apply |
| Reviewed third party | An outside publisher | Refused while the `third-party` cargo feature is off, which stays until the catalogue client (`P7`) lands. The open store is then the target |

The tier is assigned by the host from where a package came from. A package cannot claim a tier.

### Native plug-ins

Native plug-ins are accepted in the store (`D3`). They run out of process, carry the label
"Native: full access to your computer", and a person must consent explicitly before one runs. OS
confinement is a later goal (`P10`), not a listing requirement. Two maintainers review every native
listing and every native release; a bot never auto-merges one. Revoking a native plug-in terminates
its process.

## Threats and the gate that answers each

| # | Threat | Applies to | Answer |
|---|---|---|---|
| T1 | Zip bomb: tiny archive, huge output | all | **Built and tested (phase 1).** Caps on entry count, per-entry and total size, and compression ratio, enforced while streaming. |
| T2 | Path escape: `..`, absolute paths, drive letters, links, device names, trailing dots, case collisions | all | **Built and tested (phase 1).** Paths are validated by the manifest and again against the archive; links and encrypted entries refused. |
| T3 | Extra or missing files: archive holds more than the manifest lists | all | **Built and tested (phase 1).** The archive must match the manifest exactly, each file by SHA-256 and size. |
| T4 | Hostile text: bidi overrides, zero-width and control characters in names | all | **Built and tested (phase 1).** Display text is validated by character class. |
| T5 | Lookalike or homoglyph ids | all | **Built and tested (phase 1).** Ids are lowercase ASCII from a small alphabet. |
| T6 | Impersonating a first-party extension | third party | **G1 reserved namespaces.** Ids under first-party prefixes only from the first-party tier. |
| T7 | Unreadable text from a theme (low contrast) | theme | **Built and tested (phase 1).** Contrast check at install and again whenever an app loads the theme. Per-app overrides use names only the app knows, so each app re-checks its own pairs. |
| T8 | Malicious or buggy plug-in: infinite loop, memory blowup, escape | plug-in | The host's sandbox limits, plus **G6**: declared permissions, granted by the person in the host, checked by grant generation on every host call, at install and again on every load. Until the operations kit (`P2`) lands, plug-ins ship with empty permissions and no host imports. |
| T9 | Hostile resource file exploiting an importer | resource | **G8** fuzzing of each importer before its resource type ships. |
| T10 | Crash on any parser input | all | **G8** fuzz targets and a panic hunt; crate roots deny `unwrap`, `expect`, `panic`. |
| T11 | Tampered download or index | all | First party: pinned SHA-256 from the organisation's own releases. Third party: **G2** publisher signatures with pinned keys. |
| T12 | Rollback to an old vulnerable index or version; frozen stale index | all | **G3** signed index with version and expiry; downgrades and stale indexes refused. |
| T13 | A bad package that is already installed | all | **G4** revocation list checked at launcher start and before enabling; a revoked package turns off in every app. |
| T14 | Silent change of publisher key, permissions or version | third party | **G7** any such change needs a fresh yes; third-party updates never apply on their own. |
| T15 | Unknown origin of a binary | plug-in | **G9**, settled: the store builds and hosts nothing. Assets download from the author's GitHub Releases with a public source repository and commit link and a pinned SHA-256 asset digest in the signed catalogue. |
| T16 | A weak review lets a bad entry in | third party | **G10** written review checklist, disclosure and takedown path. Two maintainers review native listings and native releases; routine WebAssembly updates use bot checks and a 48-hour hold. |
| T17 | Flaws in our own unpack, sandbox host and signature code | all | **G12** independent outside review before public launch. |
| T18 | The gate is opened by accident | third party | **G5** a cargo feature off by default plus a test that fails if it is a default; **G13** opening needs an explicit decision. |
| T19 | A key or token leaks into the repository | all | Test keys only in the repo; no real signing material ever; CI scans are not a substitute for care. |
| T20 | Another local program edits the store | all | Accepted: it runs as the same user and can already edit the apps. Readers still re-validate everything they load and treat a bad file as absent. |

## Gate list (the `third-party` feature stays off until the catalogue client lands and these pass)

| Gate | What |
|---|---|
| G1 | Reserved first-party namespaces; lookalike ids refused (built: ids, tier check, index) |
| G2 | Publisher signatures, pinned and rotatable keys (test keys only) |
| G3 | Signed, expiring index metadata; no rollback or freeze |
| G4 | Revocation list that disables a package in every app |
| G5 | Tier gate behind a cargo feature, off by default, guarded by a test (built) |
| G6 | Permissions declared per app in the manifest, granted by the person, checked by generation on every host call; plug-ins re-validated at install and every load |
| G7 | No silent key, permission or version change |
| G8 | Fuzz targets and a panic hunt over every parser |
| G9 | Provenance per entry: public source link and pinned asset digest; the store builds and hosts nothing |
| G10 | Review process, disclosure and takedown; two-maintainer rule for native listings and native releases, bot checks plus a 48-hour hold for routine WebAssembly updates |
| G11 | Third-party interface, compiled only with the feature on |
| G12 | Independent outside security review before public launch |
| G13 | The decision to open the gate, by the maintainers, for that specific change |

## Not claimed

Hash pinning proves a file matches the catalogue. It does not say who made it; every listing carries a public source link and trust comes from the review and the label. Until the catalogue client lands, nothing outside the Craft organisation is installable from a catalogue. A native plug-in has no sandbox promise; its label and the consent step are what a person agrees to.
