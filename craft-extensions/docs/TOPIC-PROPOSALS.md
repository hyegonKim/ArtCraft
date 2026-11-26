# Topic proposals

Status: **working record for review, 2026-11-10.** These are the proposals from each design topic (T02–T18), in the order they were discussed, with the source evidence and the alternatives that were rejected. Later sections and their "outcome" notes amend earlier ones, so where they differ the later text wins, and [ARCHITECTURE.md](ARCHITECTURE.md) is the consolidated result. Each choice is in [DESIGN-DECISIONS.md](DESIGN-DECISIONS.md).

Code references name an app repository, a commit and a path (for example EffectCraft `ce9eaec` `crates/render/src/pipeline.rs:622-632`). They were read from local copies at those commits; upstream code has moved since. References to the earlier research notes, which are not in this repository, are kept as plain names.

## SDK layers

These are cohesive responsibilities. They do not prescribe one crate, repository or service for every row.

| Layer | Primary user | Owns | Must not absorb |
|---|---|---|---|
| Guest SDK common module | Plugin author | Typed lifecycle/results/errors, parameter/state support, host-created handles, generated call wrappers, progress/cancellation and logging helpers | Installer, signing keys, user grants, private app Rust types or ambient filesystem/network authority |
| Generated app/family contracts | Plugin author | Supported operation types, data semantics, capability requirements and compatible versions exported by the app registry/ABI profile | A second hand-maintained interface definition or unsupported internal operations |
| Host adapter support | App maintainer | Registry inventory export, principal/scope enforcement hooks, module/profile admission, contribution registration and transaction/job/artifact integration | A common document engine or permission bypass for trusted-looking packages |
| Author tooling | Plugin author and reviewer | Create/add/inspect/check/test/dev/pack/prepare-release, pinned contract inventories, deterministic inventory/digests, dependency/licence report and compatibility differences | Automatic permission insertion, publication without a release action, user-side compilation during install |
| Conformance support | Author, app maintainer and reviewer | Real adapter fixtures, denied calls, output/state/cancel/migration cases, literal expected results or justified numerical tolerances | Mock-only proof that the actual app/backend is safe or correct |
| Management client/core/store | App/launcher maintainer and end user | Identity, verified package lifecycle, catalogue fetch/authentication, app setup/grants, resolution and retained releases/resources | Algorithm SDK functions or app-specific editing/rendering semantics |

One author workspace declares its package and adds app contributions. Most authors should use generated types and normal app controls rather than understand store schemas or cryptography. App maintainers implement their own contracts using reusable support. Release tooling generates routine metadata from authoritative declarations and exact bytes.

The Rust operation registry/inventory is the recommended source of truth. Existing effect/filter ABIs also need authoritative typed descriptions; not every pixel operation is a generic command. A future WIT/component encoding is generated only when justified and verified. Avoid a second schema authority and avoid serializing large image/audio data through an unsuitable generic request payload.

## Family contracts

| Family | Normal contribution | SDK semantics required | Risks and special cases |
|---|---|---|---|
| A: transactional editing | Query selection, create/modify native objects, validate/preflight | Typed queries, durable/revision-checked handles, validated edit proposals, transaction and undo boundaries | Nested/queued rights, stale jobs, partial failure, side effects that undo cannot retract |
| B: raster/develop/material | Image filter/generator, brush/material/bake or develop operation | Colour/model/alpha/precision, bounds/bands/overlap/scale, selection and channel data, output validation | Seam equivalence, non-finite output, memory copies, input/resource retention and host-owned model readiness |
| C: temporal composition/video | Effect, transition, frame processing | Timebase/frame dependencies, keyframed parameters, seek/reset, independent instance and render-session state | Backward/random evaluation, concurrent preview/export, missing inputs, GPU/device failure |
| D: geometry/modeling | Object filter, live appearance, parametric feature | Units, object/topology schema, bounds/validation, evaluation dependencies, rebuild/state/fallback | Invalid geometry, solver cost, stale models, live-feature cache invalidation |
| E: text/page/layout | Text/story/page edit, layout/prepress validator | Durable positions/ranges, stories/styles, pages/frames/spreads, layout/reflow and output | Offset invalidation, reflow effects, document-wide work, PDF-specific object lifetimes |
| F: calculation | Custom workbook function or typed range operation | Cell types, dependency/recalculation graph, deterministic inputs/results, cycle/error contract | Recursive/cyclic work, cached dependencies, external-data permission, synchronous evaluator deadlines |
| G: real-time audio | DSP effect/instrument, MIDI/event processor | Prepare/process/reset, rate/layout/blocks, sample offsets, transport, automation, latency/tail, state and editor lifecycle | Callback allocation/locking/deadlines, worker failure, device change, offline bounce versus live processing |
| H: provider/workflow jobs | Local model/provider or explicit external workflow | Declared inputs/outputs, model/node readiness, progress/cancel, durable jobs, egress and credential brokers | Remote-only availability, data disclosure, external code/process execution, restarts and partial uploads |
| I: data/theme/resource | Theme, LUT, preset, brush set, template, localization | Typed importer and version, bounded parsing, identity/licence/readiness, semantic theme mapping | Hostile data, active choice versus install, missing models, no install-time execution |

Panels/tools/widgets, settings/keymaps/context menus, format handlers and artifacts cross these families. They use versioned registration and lifecycle contracts. They do not imply arbitrary UI code, replacement of builtin handlers or new global command authority.

## All sixteen apps

Membership and candidate contributions are design requirements. The evidence column identifies what has actually been established and where to read it; full source notes carry revisions and limitations.

| App | Families | App-specific SDK requirements | Existing foundation and investigation |
|---|---|---|---|
| EffectCraft | A, B, C | Layers/compositions, effect/keyframe parameters, premultiplied pixel/time context, frame dependencies and persistent render references | WASM/helper/effect systems and GitHub Library source; CLI/output/lifetime gaps in media audit |
| PhotoCraft | A, B, I | Layers/masks/selection, colour models and alpha, band overlap, destructive/smart filters, resource/theme mapping | WASM filter ABI and separate theme branch; smart/CLI/identity gaps in media audit |
| FilmCraft | A, C, G, I | Timeline/clips/tracks, media/frame/audio time, transitions, export jobs and timeline appearance | Command/job foundation; processing plugin host not established in media audit |
| LightCraft | A, B, I | Catalog items, metadata/develop recipes, RAW stage placement, scoped batch/export and presets | Existing commands/resource importers; no code host established in media audit |
| ArtCraft | H, I | Provider/model/media jobs, explicit egress, model/workflow readiness, credential and result-ingestion boundaries | Native provider/job architecture differs from editor registries in media audit |
| FigCraft | A, D, E | Node/layout/selection edits, transactions, panels/tools/widgets and host-written artifacts | Existing WASM commands/panels with actual policy; integrity/control/output gaps in design audit |
| VectorCraft | A, D, E, I | Object/units schemas, live appearances, precise export/format selection and persistent resources | Filter/live-effect/exporter ABI and CLI store attachment in design audit |
| PDFCraft | A, E | Page/object queries and edits, forms/annotations where supported, history/preflight and PDF writes | Facade/automation foundation, incomplete plugin/UI tracing in design audit |
| DesignCraft | A, E, I | Stories/styles/frames/spreads, units, pointer tools, reflow/prepress/export | Existing commands and journaled tools; plugin seams needed in design audit |
| DeckCraft | A, E, C, I | Slides/shapes/text, transitions, presentation subscriptions and export artifacts | Commands/CLI, separate playback lifecycle not established as plugin contract in design audit |
| SoundCraft | A, G, I | Routing, DSP/instruments, audio automation/MIDI/state, presets and editors | Existing native-format hosts do not prove confinement in domain audit |
| CADCraft | A, D, I | Entities/units, explicit headless parameters versus prompts, constraints where supported, patterns/resources | Commands and AutoLISP with separate trust concerns in domain audit |
| SolidCraft | A, D, I | Feature graph/rebuild/model state, geometry/solver boundaries and missing-feature fallback | Manifest/design foundation, executable host not established in domain audit |
| TextureCraft | A, B, D, I | Mesh/UV/texture-set/channel scopes, strokes/materials/bake/composite and resource ownership | Sessions/commands, job and resource lifetime work in domain audit |
| WordCraft | A, E, I | Durable text ranges/styles/stories, layout/validation and output | Commands/history, internal duplicate policy and untraced boundaries in domain audit |
| GridCraft | A, F, I | Typed ranges/cells, workbook queries/edits, function/recalc and macro policy boundaries | Command/calc/macro foundations; custom function contract and test-helper qualifications in domain audit |

Family C membership for presentation transitions does not equate DeckCraft to a video compositor. Family G membership for timeline audio does not make FilmCraft a real-time plugin host. The contract lists what must be discussed; an adapter advertises only implemented capabilities.

## What belongs in common libraries

Share identity, version/policy vocabulary, descriptors/inventory tooling, principal/handle mechanics, job/error/artifact envelopes, parameter/declarative UI descriptions and conformance infrastructure where actual consumers agree. Reuse craft-ext-core/store for package management, existing craft-ui drawing support and craft-keymap conflict handling. Inspect definitions before choosing concrete calls.

Keep actual operations, document models, transaction application, file format serialization, colour/geometry/time/audio conversion, render scheduling and theme token mapping in apps. Algorithm crates can be reused by an author across contributions without making their app ABIs identical.

First structure the coherent modules, then choose crate boundaries. A logical SDK split is not a mandate for one dependency per feature. A new craft-libs crate needs real multiple-app use and no dependency on application crates. No relocation of craft-extensions or new common framework is approved merely by listing these responsibilities.

## Questions to resolve in the session

For one concrete contribution, name what the author implements, what the SDK generates, what the app supplies, and which authority the host enforces. Follow it through data conversion, editing/evaluation, save/reopen, headless invocation and failure. Then test whether a second app shares actual semantics or only the name. This determines useful sharing more reliably than grouping apps by marketing category.

## T02 session proposal: contract layers and interaction styles

Status: second revision in the design session, 2026-11-08, after an end-to-end review from every actor's perspective. Not accepted. It replaces the first "two planes" proposal, which misclassified VectorCraft object filters and GridCraft functions. It applies the accepted functional default "standardize purpose-specific interfaces where real app requirements agree" and the [T01 grant decision](DESIGN-DECISIONS.md).

### Two independent axes

A contribution has a **domain** (what data it understands) and an **interaction style** (how it runs). The first proposal fused them.

| Style | How it runs | Imports | Typical contributions | Today's evidence |
|---|---|---|---|---|
| **Transform** | Host hands a scoped, typed input; plugin returns a typed output; host validates and commits | None beyond its contract; cannot reach documents, files or network | Raster filter, temporal effect, audio processor, object filter, live appearance, parametric feature, workbook function, exporter encoding | PhotoCraft `pc_filter` and VectorCraft ABI 1 already have no host imports; VectorCraft applies output in one `s.edit` and refuses foreign object IDs |
| **Session** | Plugin calls the app's generated operations under grants, inside host-owned transactions | Generated app operations, each checked against the grant and scope | Commands, panels, pointer tools, canvas widgets, validators that propose fixes | FigCraft sandbox ABI 4 with 14 host imports and real command/read restrictions |
| **Data** | No code. A typed importer reads it | None | Themes, LUTs, presets, templates, localization | LightCraft resource importers; SoundCraft track presets; PhotoCraft theme branch |

Effective authority of a runtime instance is **its style's imports ∩ the package's grant in that app**. A transform module therefore cannot use a network grant that a session contribution in the same package received. This keeps the T01 single grant sheet while staying narrow by construction.

### Three contract layers

| Layer | Contents | Owner | Shared? |
|---|---|---|---|
| 1. Common mechanics | Identity, parameters with units/ranges/animation flags, declarative UI, results and stable error codes, jobs/progress/cancel, handles and revisions, edit-proposal envelope, bounded logs | Shared crates (T03 decides placement) | All apps |
| 2. Shared domain contracts | `raster` (image regions, pixel formats, regions of interest), `temporal` (time context, frame access, instance state), `audio` (prepare/process/reset, blocks, events, latency/tail), typed resource formats such as LUTs. Candidate only: `path2d` | One shared definition per domain, each with one conformance suite | Only apps whose adapters pass that suite |
| 3. App contracts | Document operations and app-specific extensions (`effectcraft.*`, `figcraft.*`, `gridcraft.*`), generated from the app's Rust registry | The app | No |

Shared domain contracts are designed now where the domain is known to span apps: raster (PhotoCraft, EffectCraft, LightCraft, TextureCraft, FilmCraft), temporal (EffectCraft, FilmCraft, DeckCraft transitions), audio (SoundCraft real-time, FilmCraft offline). An app makes a contract available only after its adapter passes the suite. An unforeseen need starts as an app extension and is promoted when a second app needs the same meaning.

### Domain contract shape

Core plus named optional extensions, following OpenFX (host/plugin describe depths, components, premultiplication, tiling, temporal access and regions of interest) and CLAP (small core, negotiated extensions; also the audio model SoundCraft already hosts).

- The **plugin declares** supported pixel/colour models, accepted alpha/transfer formats, required extensions and optional ones it can use.
- The **app advertises** contract versions and extensions it implements, plus limits such as maximum region.
- **Compatibility** per contribution per app is computed at catalogue display, install, enable and load from the same function: available, or unavailable with a reason.
- **Optional features** are queried at runtime from a host-provided feature set; a plugin must behave correctly without them.

### Host obligations (every app adapter)

1. Convert native data to a format the plugin accepted, using shared fixture-tested conversion helpers; never silently convert colour models (CMYK, Lab).
2. Supply the requested input region, fall back to whole-image when within the workload profile, or refuse before work starts. Never produce seams silently.
3. Evaluate keyframes and convert spatial units to render pixels; the plugin receives plain values plus render scale.
4. Validate output dimensions, finiteness and object IDs; a failure is a terminal failure, never a pass-through success. A labelled preview fallback is allowed.
5. Commit through the app's own transaction/undo; save live features with exact release references (T01).
6. Isolate instances: stateless transforms may be pooled and parallelized; stateful ones get one instance per feature instance per render session, with reset on seek.

### Plugin obligations (checked by SDK tests and review)

- Deterministic output for the same input, parameters and context; no relaxed SIMD or other nondeterministic features in transform modules.
- **Tile invariance**: whole-image and tiled output agree (exactly, or within a declared numerical tolerance). The conformance suite checks this; it exposes an under-declared region.
- Honest declarations: unsupported formats are declared, not mishandled.

### Cross-app result consistency

Promise: the same contract input, parameters and context give the same output in every app. Not promised: identical appearance across apps, because each app's colour pipeline, working space and compositing are app-owned. Plugins needing a particular working space declare it and the host converts or refuses.

### Evolution and governance

- Each domain contract has named maintainers drawn from implementing apps; a change requires their review and an updated conformance suite.
- Minor versions add optional items only. A major version is a new contract an app can implement alongside the old one.
- Apps advertise exact contract versions; the SDK targets a minimum version; the catalogue shows per-app compatibility from those records.
- Existing ABIs remain legacy app profiles behind adapters (`effectcraft.wasm-effect@1`, `photocraft.wasm-filter@1`, `vectorcraft.wasm@1`, `figcraft.sandbox@4`). No automatic conversion of old modules.

### Known exclusions and dependencies

- SoundCraft's existing in-process CLAP/VST3/AU hosting is native third-party code outside the sandbox promise. It stays a separate, explicitly labelled mechanism until T06 decides a confined backend; the Craft audio contract does not inherit its trust.
- CADCraft AutoLISP is an unconfined existing surface; no package code runs there before T06 confinement.
- GPU, interpreter performance and backend choice are T06/T16. Saved-feature missing/repair behaviour is T13. Generated layout of crates and SDK is T03; operation signatures are T04.

### Worked example

Ink & Grain ships one `raster.filter@1` transform module referenced by EffectCraft and PhotoCraft contributions, and one FigCraft session module for "Halftone fill", both built from a shared `dot_layout` crate. A later VectorCraft "halftone pattern" object filter would be a transform against VectorCraft's object contract (or `path2d` if promoted), not the raster module.

## T03 session proposal: SDK, toolchain and author experience

Status: proposed in the design session, 2026-11-08, after an end-to-end review from author, reviewer, app developer, contract maintainer, end user and CLI perspectives. Not accepted. Builds on the T01 grant decision and the accepted T02 contract structure. No SDK exists today; craft-extensions has core/store only (Jeremy's d7: standalone contract/store workspace).

### Placement: one plugin-platform workspace

Keep every plugin-platform crate in **craft-extensions**: core, store, contract inventories, generator, guest SDK, host support, conformance and author CLI. One repository gives atomic changes across manifest, validator, guest and host, one version line and one CI. This follows the existing decision to reuse craft-extensions and Jeremy's d7. It corrects the earlier T02 hint that domain contracts might live in craft-libs. App-specific glue stays in each app repository.

| Crate (names illustrative) | Used by | Contents |
|---|---|---|
| `craft-ext-core` (exists) | Everyone | Manifest, identity, compatibility, the **one validator** used by SDK, catalogue bot and installer |
| `craft-ext-store` (exists) | Apps, launcher, `craft-ext dev` | Installed releases, setup, grants, readers |
| `craft-ext-contracts` | Generator, host, SDK | Hand-written Rust definitions of shared domain contracts plus checked-in generated inventories of app contracts, all in one language-neutral inventory format |
| `craft-guest` | Authors | Common mechanics: parameters derive, results/errors, progress/cancel, logging, entry-point macros, custom-section emission |
| `craft-guest-<contract>` | Authors | Generated typed bindings, one crate per domain or app contract (`craft-guest-raster`, `craft-guest-figcraft`). Crate major = contract major |
| `craft-ext-host` | App developers | Engine admission profile, module loading, linker built from descriptors, invocation context and principal, shared conversion helpers, adapter traits per domain contract |
| `craft-ext-conformance` | App developers, authors, reviewers | Domain-contract suites as a library that apps run in their own `cargo test`; reference host for transforms |
| `craft-ext` (CLI) | Authors, reviewers | create, add, inspect, build, check, test, dev, pack, prepare-release, and verify for reviewers |

### Contract pipeline (single source, generated consumers)

1. **App contracts**: the app's Rust registry marks deliberately exposed operations with descriptor metadata. An app `xtask export-contract` writes `<app>/<contract>@<version>.json`.
2. App CI diffs the export against the last published version: removed or changed operations without a major bump fail, the same approach as `cargo-semver-checks` and protobuf `buf breaking`. CI work goes through craft-common-actions under house rule 11.
3. A reviewed PR copies the inventory into craft-extensions. The generator emits the guest crate and reference docs. Generated code is committed and CI fails if regeneration differs (rust-analyzer/AWS SDK codegen practice).
4. **Domain contracts** are hand-written Rust in `craft-ext-contracts` and export the same inventory format, so tooling treats them identically.
5. Guest crates are versioned so Cargo's semver resolution gives authors compatible minors automatically; `Cargo.lock` pins the exact inventory used, and `pack` records inventory digests in the manifest.

Publishing guest crates to crates.io (giving docs.rs reference docs) is the recommended distribution, but it is a publication decision for the user and maintainers, not authorized here. Git tags on craft-extensions are the interim alternative.

### Author declarations: code for behaviour, one file for authority

- **`Craft.toml`** at the workspace root holds only facts a reviewer must read as a whole: package UUID, display name, version, licence, publisher claim, links, and **permissions per app** (matching the T01 grant unit).
- **Contributions and parameters live in code**: `#[craft::transform(contract = "raster.filter@1", id = "halftone", apps = ["photocraft", "effectcraft"])]` on a type, and `#[derive(craft::Params)]` on its parameter struct (units, ranges, animatable, labels, localization keys). The macros write descriptors into a WebAssembly custom section; `craft-ext pack` reads it from the compiled module, so manifest and code cannot drift. PhotoCraft's `pc_manifest` export is precedent for module-carried metadata.
- **Permissions are never inserted automatically** (accepted). `check` computes required capabilities from the module's actual imports and the inventory, fails when one is undeclared, warns when one is unused, and prints the exact line to add.

### Author toolchain

- Rust first, `wasm32-unknown-unknown`, toolchain pinned by `rust-toolchain.toml` in the template. The template's `.cargo/config.toml` sets the target features the apps' wasmi engine profile accepts; `check` validates used features and imports against each target app's engine profile. Recent Rust enables some WebAssembly proposals by default on this target, so this must be verified against the actual wasmi versions before the template is fixed.
- Deterministic builds: path remapping, locked dependencies, normalized zip ordering/timestamps/permissions. A reviewer can rebuild from a source tag and compare digests (optional evidence, not a review substitute).
- Supply chain: `cargo-deny` for licences and advisories, a dependency inventory/SBOM produced by `pack`, licence policy from T17.
- Other languages later, only with generated and verified bindings; the inventory format is language-neutral.

### Testing at three levels

| Level | Runs | Proves |
|---|---|---|
| Unit | Native `cargo test` of algorithm crates | Algorithm behaviour, fast |
| Contract | `craft-ext test`: module in `craft-ext-host` with the same engine configuration as apps; domain conformance (determinism, tile invariance, declared formats, parameter ranges, cancellation) and literal fixtures with snapshot comparison | The compiled module meets the shared contract without any app installed |
| App | The app's headless CLI loads the packed package against fixtures (`<app>-cli plugin test`), like VS Code extension tests running in a real VS Code | Session contributions and the actual adapter; final acceptance |

Apps without a headless plugin test entry cannot offer session contracts to authors; that becomes part of each app's adapter obligation.

### Development loop

`craft-ext dev --app photocraft` builds, packs, installs into a separate labelled development store, starts or attaches to the app pointed at that store, streams plugin logs and reloads on change. Transforms swap between invocations; session contributions dispose UI and instances first. Development grants are isolated from the user store. Release app builds treat development packages as local installs with all normal checks.

### Release and submission

`craft-ext prepare-release` builds deterministically, runs check and test, writes the zip, inventory/SBOM, and the catalogue record `releases/<version>.toml`, and compares with the previous release: removed or renamed contribution IDs, parameter/state schema changes without migration, and widened permissions are reported prominently. A template GitHub Actions workflow for authors builds on a tag, uploads the asset and adds GitHub artifact attestations. Submission is an explicit author PR; the tool never publishes on its own.

### Trust boundary

The SDK is a convenience, never a trust anchor. A hand-made package gets the same validation from `craft-ext-core`, the same import checks at load and the same grants. The catalogue bot, `check` and the installer call the same validator.

## T04 session proposal: one operation path for GUI, CLI, automation and plugins

Status: proposed in the design session, 2026-11-08, after an end-to-end review. Not accepted. Builds on T01–T03.

### Evidence that shapes it

- Every audited app has a static command table (`cmd!`/`query!`/`CommandSpec`) with loosely typed parameters described by strings, and an `execute(id, params)` with **no caller identity** (EffectCraft `dispatch.rs:9`; six domain apps listed in the domain audit F2).
- Policy exists only at some entry points: FigCraft checks its denylist/allowlist inside the plugin host (`plugins.rs:148-181`) while its control channel's `engine.execute` reaches `execute_as` directly. PhotoCraft and TextureCraft have real headless filesystem modes and `authorize_*` hooks.
- Domain apps share a snapshot/label/rollback transaction pattern (domain audit F5); FigCraft and VectorCraft already make one plugin run one undo step.
- Several apps expose every command over MCP or a control channel; some channels have no authentication (EffectCraft, LightCraft, SoundCraft).

### The structure

**1. Extend each app's existing registry; do not add a second one.** Each descriptor gains:

| Field | Meaning |
|---|---|
| `args`, `result` | Rust types with `serde` and `schemars` derives; JSON Schema generated from them feeds SDK, CLI help and MCP tool schemas |
| `kind` | `Query` (reads, no undo), `Edit` (one transaction), `Job` (asynchronous, progress/cancel, commits through an Edit), `Effect` (external side effect that undo cannot retract: file write, network, process) |
| `exposure` | Set of caller kinds allowed: `Gui`, `Cli`, `Automation`, `Plugin`. **Plugin exposure is opt-in per operation**; management operations (install, enable, grant, settings that change authority) are never plugin-exposed |
| `requires(resolved) -> CapabilitySet` | Maintainer-written function over **resolved** targets and arguments, unit-tested; covers parameter-dependent rights (S08) |
| `interaction` | `None`, `Optional`, `Required`; headless callers missing input get `InputRequired` listing what is missing |
| `contract`, `since`, `deprecated` | Version metadata for the exported inventory |

**2. One dispatcher with a mandatory invocation context.**

```text
fn execute(&mut self, inv: &Invocation, op: OpId, args: Args) -> Result<Outcome, OpError>

Invocation {
  principal: Principal,          // User(Gui) | Cli(session) | Automation(client, scope)
                                 // | Plugin(app, package, release, contribution, grant_generation)
  origin: Vec<Principal>,        // chain for nested/queued work; never upgraded
  scope: CapturedScope,          // document, revision, selection captured at start
  deadline, cancel: CancelToken,
  transaction: Option<TxnId>,    // nested calls join the outer transaction
}
```

The old `execute(id, params)` signature is removed, so a new entry point that bypasses policy does not compile. Every caller (menus, keymap, panels, CLI, MCP, control channel, scripts such as GridCraft macros and CADCraft AutoLISP, plugin imports) builds an `Invocation`. Migration is mechanical: existing callers pass their real principal; all existing descriptors default to no plugin exposure.

**3. One authorization function.** `authorize(inv, descriptor, resolved) -> Allow | Deny(reason) | NeedsConsent`. Order: exposure for caller kind, then argument validation and resolution, then `requires(resolved)` against the principal's effective authority (T02: style imports intersected with package grant), then scope (targets must lie inside the captured scope). `NeedsConsent` only returns to interactive user callers; plugins and headless callers get `Deny` with a reason. All denials happen before any side effect.

**4. Handles instead of paths and pointers.** Plugins receive opaque, revision-carrying handles for nodes, layers, ranges and jobs, and **artifact roles** for outputs; the host chooses and writes destinations. User-chosen inputs arrive as host-issued file tokens, like browser File System Access handles, macOS security-scoped bookmarks and WASI preopens. A stale handle yields `StaleDocument`.

**5. Transactions owned by the dispatcher.** An `Edit` opens a transaction (existing snapshot/rollback), runs, validates, commits one labelled undo step showing origin ("Halftone fill · Ink & Grain"), and rolls back on error, trap, cancel or budget exhaustion. Calls nested inside a plugin invocation join it, so one plugin command is one undo step. Plugins cannot reach undo/redo or history operations.

**6. Two execution shapes for session plugins.**
- **Short command**: runs with an exclusive document transaction under a strict time/fuel budget.
- **Long work**: a `Job` reads from a captured revision, computes off the editing thread, then commits an edit proposal with an optimistic revision check. If the document changed, the app's operation decides: reject with `StaleDocument` or apply where provably independent. No plugin callbacks run inside another transaction (no reentrancy).

**7. Data paths.** Control-plane arguments are JSON, validated against the schema with unknown fields rejected and size/depth limits. One encoding keeps CLI, MCP and plugin validation identical; generated wrappers hide it. Bulk image/audio/geometry data uses T02 handles and buffers, never serialized command payloads.

**8. Generated surfaces.** From the same inventory: SDK bindings (T03), CLI `ops list --json`, `op <id> --args`, generated help and existing commands kept as aliases, MCP tool schemas, reference docs. Stable error codes map to CLI exit codes and MCP errors.

**9. Nested plugins and confused deputies.** A plugin that applies another installed transform passes data only; the inner code runs under its own contract and grant. Authority never transfers between principals in either direction. Plugin-to-plugin APIs remain a future family.

### Out of scope here

Authentication and scope of automation sessions (T15), consent UI and grant storage (T05), runtime budgets and traps (T06), job persistence and artifacts across restarts (T15/T16). Unauthenticated control channels are an existing issue that the dispatcher cannot fix; it only guarantees that an authenticated channel's principal is enforced.

## T04 revision after the all-perspective review

Status: revision proposed 2026-11-09 after walking the flow as end user, plugin author, transform and session plugin, fresh CLI, attached CLI, MCP agent, in-app script, app maintainer, reviewer and support. Evidence from EffectCraft `ce9eaec` and FigCraft `0e86f359`. Amends the T04 proposal above; not accepted.

### New evidence

- **Authority hides in generic commands.** EffectCraft `prefs.set` can change the Scripting permissions; `crates/automation/src/confine.rs:205-230` special-cases it and batch `$N` references by hand. Per-operation exposure alone would miss this.
- **UI enablement is used as a gate.** `Session::execute` (`crates/engine/src/session/dispatch.rs:9`) refuses a command when `spec.enabled` fails, unless explicit targets are passed. Availability and authorization are mixed.
- **Commands feed other systems with no origin.** `execute` appends to a 10 000-entry journal and calls `learn_observe` (habit learning) for every caller.
- **FigCraft's control channel** reaches `engine.execute` for any command (`crates/mcp/tests/it/attached.rs:129`), outside the plugin host's checks.

### Amendments

1. **Separate three questions the dispatcher answers, in order**: *Is this caller kind allowed this operation?* (exposure), *Is it permitted with these resolved arguments?* (authorization), *Does it make sense right now?* (availability, the existing `enabled`). Availability is a UI hint and an error message, never a security check, and explicit targets never skip authorization.
2. **Authority-changing state is its own operations.** Settings that change what callers may do (scripting permissions, grants, enablement, install, trusted folders, update sources) move out of generic `prefs.set` into `Management` operations. Generic setters reject those keys. Management operations are exposed to `Gui` only, plus `Cli`/`Automation` with a management scope (Q1). This removes the hand-written special cases in `confine.rs`.
3. **Side effects run after commit (outbox).** Inside a transaction, an `Effect` (file write, network, process launch) is queued, not performed. It runs only after the edit commits and is dropped on rollback. Results that undo cannot retract are reported separately ("Exported 3 files"). Standard transactional-outbox practice.
4. **Origin travels everywhere the command goes.** Journal entries, undo labels, habit learning, logs and events carry the `Principal`. Plugin and automation calls do not train user habits. Replaying a journal or macro runs under the *replayer's* principal; plugin-originated entries are marked in the replay preview.
5. **Batches are one invocation.** A batch (EffectCraft `batch`, MCP multi-step) is authorized step by step with the same principal and runs as one transaction; `$N` references are resolved before authorization, so confinement does not need its own batch rules.
6. **Events after commit, never inside.** Plugins observing document changes receive a committed-change notification with the new revision, after the transaction. Prevents re-entrancy and loops; a plugin reacting with an edit makes a new invocation with itself as origin, and the origin chain is capped.
7. **Compatibility is checked at load, not mid-call.** A session plugin's imported operations are resolved against the app's exported inventory when it loads. A missing or major-mismatched operation makes the contribution unavailable with a reason, instead of failing halfway through a user's edit.
8. **Hot paths stay fast.** The `Gui` principal has full user authority, so authorization is a constant-time check. Slider drags keep the existing merge-key undo folding. Per-call cost is bounded for plugins by the T06 budget.
9. **Where the shared types live.** Following T03 A, the `Invocation`, `Principal`, descriptor, error and authorization scaffolding live in a small craft-extensions crate (illustrative name `craft-ext-ops`) that apps depend on even without plugins. Operation definitions and data stay in apps.
10. **Migration per app, in this order**: add `Invocation` and the new `execute` signature with every existing caller passing its real principal; give every descriptor `kind` and default exposure (`Gui | Cli | Automation`, no `Plugin`); split authority-changing settings into management operations; add schemas; then open selected operations to plugins. Each step is shippable on its own.

### Alternatives rejected

| Alternative | Why not |
|---|---|
| Hand-written separate plugin API per app (FigCraft today) | Drifts from CLI/GUI behaviour and duplicates validation; the control channel already bypasses it |
| WebAssembly Component Model / WIT interface per operation | wasmi, the engine all apps use, does not run components; would also split validation from CLI/MCP |
| Pure object-capability passing for everything | Strongest model, but a rewrite of every app's command layer; handles and captured scope give most of the benefit |
| Policy only at each entry point (today) | Every new entry point must remember the checks; FigCraft and EffectCraft already show the gap |

## T05 session proposal: permissions and consent

Status: proposed 2026-11-09 after an all-perspective review (end user, plugin author, transform and session plugin, reviewer, app maintainer, fresh CLI, attached agent, support). Builds on T01 (grant per package per app, enablement per contribution), T02 (style imports intersected with grant) and T04 (resolved-argument authorization, management operations). Not accepted.

### Evidence

- craft-extensions `a6c5fae`: `Manifest.permissions: Vec<String>` must be empty in version 1 (`crates/ext-core/src/manifest.rs:93,174`). No grant store exists.
- FigCraft `0e86f359`: manifest `permissions {access: read|write, commands: [...], saveFile}`; install confirmation shows them; read-only and command allowlists are enforced, and `saveFile` yields one host save dialog (`crates/engine/src/plugins_tests.rs:132,458-467`).
- EffectCraft `ce9eaec`: one global Application Scripting setting (file and network access, warning before executing files) that scripts cannot change (`crates/engine/src/settings/prefs/sections.rs:55-66`).
- PhotoCraft `655295b2`: per-command `authorize_*` checks for automation, render and smart sources (`crates/automation/src/workspace.rs:327-398`).

Each app invented its own vocabulary; none is shared, none scales to sixteen apps.

### 1. Vocabulary: a small fixed set of permission families with scopes

A permission is `family + access + scope`. Families are shared by every app; operations map onto them through `requires(resolved)` (T04), so the number of permissions stays small however many operations exist.

| Family | Access | Scopes | Plain sentence (host-written) |
|---|---|---|---|
| Document | read, edit | selection, current document, all open documents | "Read and change the selected layers" |
| Files | read, write | picked item (one-time), picked folder (remembered), own storage | "Save files where you choose" |
| Network | connect | listed https domains; never loopback, private ranges or the app's own channels | "Connect to api.example.com" |
| Account | use | one named provider, brokered by the host | "Use your Example account; it never sees your password" |
| Clipboard | read, write | on a user action only | "Read the clipboard when you paste" |

Free, never asked: computing on its inputs, returning results, progress and logs, bounded private storage, its own UI placement (that is enablement, T01).

Not grantable to WebAssembly plugins: processes, native code, raw sockets, other plugins' storage, management operations. Existing native helpers (EffectCraft helper-host) stay a separate legacy profile for T06.

App-specific needs (LightCraft catalogue collections, GridCraft external data) are new **scopes** of an existing family where possible; a genuinely new family needs a host-written sentence and is promoted to shared on a second consumer (same rule as T02).

### 2. Required and optional

- **Required** rights must be granted before the contribution can be enabled.
- **Optional** rights leave everything else working; a call without one returns `Denied(needs_permission)`, which the SDK forces authors to handle.
- **Picking is permission** (powerbox): when the user picks a file or folder in a host dialog, that pick is the grant for that item. No separate prompt. Same as macOS powerbox, browser File System Access and Android's storage picker.

### 3. When the user is asked

| Moment | What happens |
|---|---|
| Setup in an app (T01) | One sheet: the union of required rights in plain sentences, optional rights as toggles, off by default. Combination warnings computed by the host (document read + network: "could send your document to api.example.com") |
| First use of an optional right | Only during a user-started action in the GUI: a host-drawn prompt naming the plugin and the exact scope. Never inside plugin-drawn UI |
| Update that widens rights | The new release stays staged; the old one keeps working until the user accepts (store mechanics in T08). Narrower or equal rights carry over silently under verified continuity |
| Headless, CLI run, agents, render queue | Never prompt. Missing right is an explicit failure with the exact grant to add |

Author-supplied purpose text is shown as a quoted, length-limited claim beneath the host's sentence, never instead of it.

### 4. Grant record and live enforcement

```text
Grant {
  app, package_uuid, source_binding,     // T07 identity; continuity for updates
  rights: Set<Permission>,               // normalized family/access/scope
  tokens: Set<PickedItemToken>,          // remembered folders
  generation: u64,                       // bumped on every change
  granted_release_digest, granted_at,
}
```

Stored in craft-ext-store, one per (app, package). Every runtime instance holds the generation it started with; each host call compares it. Revocation therefore takes effect at the plugin's next host call; running edits roll back and jobs cancel. Queued jobs check the grant when they start, not when queued.

Grants live with the user on that machine, never in projects. Opening a project that uses a plugin needing rights asks in setup; a headless render with a missing right fails explicitly, never drops the effect.

### 5. Managing grants

- GUI: per-app plugin manager shows each package's rights, last use and a revoke per right. Disable keeps grants; uninstall deletes them.
- Fresh CLI (the user, T04 Q1): `<app>-cli plugin grant|revoke|permissions`. Grants are management commands, separate from running work: no `--allow-all` on execution commands.
- Attached agents: management scope approved in the app (T04 Q1).
- Local log of grant changes and denials (privacy in T16).

### 6. Effective authority (unchanged formula, now concrete)

`style imports ∩ grant.rights ∩ host-supported ∩ invocation scope ∩ policy`. Policy is an optional machine-wide plugin policy (for example "no network for plugins"), read-only to plugins and agents.

### 7. Migration of existing systems

| Today | Becomes |
|---|---|
| FigCraft `access: read/write` | Document read/edit on current document |
| FigCraft `commands: [...]` | Kept in its legacy app profile; new packages use plugin exposure plus families |
| FigCraft `saveFile` | Files write, picked item |
| EffectCraft Application Scripting | Scripts as their own principal with the same families; changing them is a management operation |
| PhotoCraft `authorize_*` | `requires(resolved)` on the descriptors |
| craft-extensions `permissions: Vec<String>` (empty in v1) | Version 2 manifest: per-app families and scopes |

### Typical outcome

A transform such as a halftone or glow asks for nothing: setup says it cannot reach files, network or other documents. A FigCraft panel that edits the selection shows one sheet. A plugin that uploads renders shows one sheet with the account and domain, plus the combination warning.

## Open-ecosystem direction (2026-11-09) and its effect on T01–T05

User direction: plugins may use the CLI and its tools; do not restrict what the community builds; plugins cannot change built-in features; plugins are free and open source; store plugins must be reviewed as safe and not malicious; store plugins are MIT or Apache-2.0.

### Interpretation

- **"Use the CLI and its tools"** means a plugin can call every operation the app's CLI and MCP tools offer, in process, through the T04 dispatcher, with the plugin as principal. It does **not** mean launching `<app>-cli` as a process: a process runs native code with the user's full power, outside the sandbox, and could change built-in features and settings. Same capability, kept inside the checks.
- **"Cannot change main features"** is the T01 protected core made concrete. Plugins can *use* built-ins and *add* new things; they cannot replace, remove, disable, re-bind or alter: built-in commands, tools, effects, menus and shortcuts; default settings and app preferences; built-in file-format handlers; undo/history; updates; plugin management and permissions. They edit documents, which is their purpose.

### Changes to earlier topics

| Topic | Before | Now |
|---|---|---|
| T04 exposure | Plugin exposure opt-in per operation | Default on: an operation exposed to `Cli` or `Automation` is exposed to `Plugin` unless marked `Management` or `Core` (changes built-in behaviour) or `Internal`. App maintainers no longer gate the community operation by operation; they only mark the protected ones, and CI checks every operation has a classification |
| T05 | Families as user-facing permissions | Kept; the families now cover the whole CLI surface through `requires(resolved)`, so permissions describe *what data and where*, not *which features*. Nothing the CLI can do on a document is unavailable to plugins; it is only disclosed and scoped |
| T02 | Session style calls selected app operations | Session style calls the full exposed operation surface |
| T09/T17 store | Licence list open; proprietary and restricted releases designed for | Store accepts only open-source plugins under MIT or Apache-2.0 (either or both), with a public source repository. Proprietary distribution is not a store goal |

### Store admission: how "safe and not malicious" is checked

No review can prove absence of malice, so admission combines review with limits that hold even if review misses something.

1. **Open source required**: public repository, licence file MIT and/or Apache-2.0, SPDX expression in `Craft.toml`.
2. **Reproducible build**: the catalogue bot rebuilds the tagged commit with the pinned toolchain (T03 deterministic builds) and the digest must match the submitted asset. What reviewers read is exactly what users run. Was optional evidence in T03; now mandatory for the store.
3. **Automated checks** (bot, every release): `craft-ext-core` validator; WebAssembly import and feature scan against declared style and permissions; `cargo-deny` licences and advisories; dependency inventory; declared network domains; conformance tests; permission diff against the previous release.
4. **Human source review** of every release, reading the diff since the last reviewed release. Higher-risk combinations (document read with network, account use) need a second reviewer.
5. **After publication**: user reports, revocation of a release digest that takes effect at next launch or online check (T10), and the runtime limits of T05/T06 regardless.

## T06 session proposal: runtime and safety

Status: proposed 2026-11-09 after an all-perspective review (end user mid-edit and mid-render, plugin author debugging, malicious author, app maintainer, reviewer, support). Builds on T02 (styles, determinism), T04 (dispatcher, transactions) and T05 (grants with generation). Not accepted.

### Evidence

All three existing hosts already use the same engine, **wasmi 2** (pure-Rust interpreter), with fuel and `StoreLimits`, but each configured it differently:

| | EffectCraft `ce9eaec` `crates/plugin/src/wasm.rs` | PhotoCraft `655295b2` `crates/plugins/src/runtime.rs` | FigCraft `0e86f359` `crates/plugin/src/sandbox.rs` |
|---|---|---|---|
| Fuel | 50 M base + 20 k per pixel | 50 M base + 4 k per sample | 1 G per run, +10 k per host call, +1 per byte copied |
| Wall time / cancel | None | Fuel given in 20 M slices with a deadline check between slices (resumable calls) | None |
| Memory | 1 GiB per instance | Per instance **and total across parallel bands**; band parallelism limited to fit | Per instance; 1 memory, 4 tables, 100 k elements |
| Recursion, strict limits | Not set | `enforced_limits(strict)`, recursion depth | Not set |
| Imports | None allowed | ABI imports | Fixed import table |
| Instance reuse | Pooled across calls (`pool: Mutex<Vec<Inst>>`) | Per run | Per run |
| wasmi features | `deterministic` | `stable` | `stable` |

EffectCraft also runs native **external helpers** as child processes (`crates/helper-host`), designed for crash containment (an OpenFX host, for example), not security.

PhotoCraft already has the most complete pattern; the shared host should start from it.

### 1. One engine profile, shared

`craft-ext-host` owns one wasmi configuration used by every app: fuel on, strict enforced limits, recursion cap, a pinned list of allowed WebAssembly features (verify against wasmi 2 before fixing the SDK template, T03), no threads, deterministic mode for transforms (T02 obligation). Module validation at load: size, function, table, memory and import caps; unknown imports refuse the module. Validated modules are cached per release digest. Apps cannot loosen the profile; they choose budget numbers within it.

Engine-neutral host trait, so a second engine can be added later behind the same contract (Q1).

### 2. Budgets

| Budget | Mechanism | Why |
|---|---|---|
| Instructions | Fuel scaled by work size (per pixel, sample, cell, node) | Deterministic, same on every platform including web |
| Wall time and cancel | PhotoCraft's fuel slices with deadline and cancel check between slices | Responsive cancel and a real "took too long" without threads or signals |
| Memory | Per-instance cap plus total across parallel instances; parallelism reduced to fit | Many render threads cannot multiply memory |
| Host calls | Fuel per call plus per byte copied (FigCraft) | A plugin cannot spin the host |
| Output | Size and shape validated by the contract before use | Garbage never reaches the document |

Profiles (numbers measured in T16, not guessed here): **preview** (short, keeps playback smooth), **final render** (generous), **session command** (short, holds the document), **job** (long, cancellable). The GUI offers "Allow more time" per plugin, recorded like a grant; headless runs take an explicit flag.

### 3. Instances

- **Transforms**: one instance per render thread; an instance is reused only within the same document and job, and reset otherwise. EffectCraft's cross-call pool would let state from one document reach another and breaks the determinism promise; it changes to per-job reuse.
- **Session plugins**: one instance per package per app window, on a worker thread, never the UI thread. Panels draw from the last committed state, so a busy plugin never freezes the app.
- A trapped or over-budget instance is discarded, never reused.

### 4. Failure contract

| What happens | Result for the user | Document |
|---|---|---|
| Crash (trap) | Message naming the plugin; in preview the effect shows an error badge and passes the frame through unchanged | Edit rolled back |
| Too slow / too much memory | "Took too long" with "Allow more time" in the GUI | Rolled back |
| Invalid output | "Returned an invalid result" | Nothing committed |
| Forbidden call | Structured denial the plugin can handle; not a crash | Unchanged |
| Permission revoked mid-run | Next host call fails; job cancelled | Rolled back |
| Final render or export with a failed plugin | **Q3** | — |
| Repeated failures | After several consecutive failures in a session the contribution pauses for the session with a notice; it stays installed and enabled | — |
| App crashed while loading plugins | Next start offers safe mode with plugins off (detail in T16) | — |

The host never panics because of a plugin: wasmi is safe Rust, every boundary returns errors, and the existing command guard remains a last resort.

### 5. Proof

`craft-ext-conformance` includes a hostile-module suite: infinite loop, memory bomb, deep recursion, huge tables, unknown import, trap in every callback, invalid outputs, calls after revoke, cancel during work. It runs in CI on every supported desktop target and the web build. Protection is claimed only where the suite passes.

### 6. Native, GPU and audio

- **Native code** (EffectCraft helpers, OpenFX hosts, VST): cannot be sandboxed by wasmi; a child process contains crashes, not malice. **Q2.**
- **GPU**: plugins call host GPU operations (built-in kernels exposed as operations). Plugin-supplied shaders are a later backend after driver-hang and resource evidence.
- **Real-time audio**: wasmi cannot promise audio deadlines; stays excluded (T02) until SoundCraft evidence.

### T06 outcome (2026-11-09)

User chose wasmi only (A), native plugins in the store after review (C) and failing exports on plugin failure (A). Native safeguards: out of process only through a versioned helper protocol (EffectCraft helper-host design), never loaded into the app; the store builds native binaries from the tagged source for each platform, with no author-uploaded binaries or prebuilt blobs; two reviewers with a full first review; a distinct "Full access to your computer" label and consent screen; updates need user confirmation; revocation kills the process; OS confinement (macOS App Sandbox, Windows AppContainer, Linux Landlock or bubblewrap) applied per platform as proven, enforcing the declared permissions. Until confinement is proven on a platform, the protected core holds for native plugins only through review.

### Store role corrected (2026-11-09)

The user corrected the store's role: **the store is a listing, not a builder, host or guarantor.** Plugins are fetched from the author's GitHub and Releases. Before listing, maintainers check the package conforms to Craft formats and rules and run a light test. There are no restrictions on what a plugin may do and the project takes no responsibility for it. This supersedes, in the open-ecosystem section and the T06 outcome above: mandatory reproducible rebuild by the store, human source review of every release, two reviewers, store-built native binaries and OS confinement as a store requirement. It matches the earlier settled distribution direction (publisher GitHub Release assets and a reviewed metadata repository).

What remains, because it is cheap and protects users without restricting authors:

| Kept | Why |
|---|---|
| Catalogue record pins the exact asset digest that was checked | If the author's release or account is later changed or compromised, the app refuses the swapped file instead of installing unchecked code |
| Bot checks: manifest validator, licence file MIT/Apache-2.0, public source link, digest, load and smoke test in the reference host | This is the "conforms to our formats and rules" check and the light test |
| Labels: "Native: full access to your computer", "Not reviewed" for file installs | Users know what they are running; no responsibility claimed |
| Delisting and digest revocation | The project can remove a listing it learns is malicious without taking on review duties |
| Runtime sandbox and permissions for WebAssembly plugins (T05/T06) | Protects users by design, costs authors nothing, and is what makes light review acceptable (Q asked) |

## T07 session proposal: package identity and compatibility

Status: proposed 2026-11-09 after an all-perspective review (author renaming or moving a repo, author selling a plugin, impersonator, user with an old project, user on an older app, store maintainer, app maintainer changing a contract). Builds on T01 lifetimes, T03 contract versions, T06 engine profile and the corrected store role. Not accepted.

### Evidence

- craft-extensions `a6c5fae`: package `id` is a lowercase reverse-domain string (`org.example.tone`) from a tiny alphabet so look-alikes cannot exist (`crates/ext-core/src/id.rs`); first-party prefixes are reserved; `version` is strict `major.minor.patch`; `compat` per app has `min_app` and a plug-in `interface` number; `provenance` has source and commit; every file is listed with SHA-256; unknown fields are ignored.
- EffectCraft `ce9eaec`: plug-in manifest `id` is reverse-domain (`org.example.posterize`), `ec.` reserved, `api` 1 or 2, free-text `version` (`crates/effects/src/plugin.rs:168-183`). Projects save only `Effect { effect: "<id>" }`, with no package or version (`crates/project/src/props.rs:183-186`).
- FigCraft `0e86f359`: reverse-domain `id`, `api` 1 to 4.
- The design drafts used a package UUID (`pkg:<uuid>/<contribution>`); no app uses one.

### 1. Identity: one readable ID, ownership checked by the store

- **Package ID** stays the existing reverse-domain form. Its namespace is verified the way Flathub does: `io.github.<owner>.<name>` is accepted when the listing's GitHub repository belongs to `<owner>`; any other domain is verified once by a maintainer (for example a DNS TXT record or a file on the domain). First-party prefixes stay reserved. No UUID.
- **Contribution ID**: `<package-id>/<contribution>`, for example `io.github.ink.grain/halftone`. Unique within the package and never reused for a different behaviour.
- **Release**: package ID + version + the asset digest pinned in the catalogue record.
- IDs never change. Display names, repository URLs and descriptions can.
- A local file using an ID that is installed from the store is refused unless the user removes the store copy first (no silent takeover).

### 2. Moves, renames and owner changes

| Event | Result |
|---|---|
| Repository renamed or moved on GitHub | Maintainer updates the listing URL; the ID stays; users notice nothing |
| Plugin renamed | Display name changes; ID stays |
| Ownership transferred (sold, handed over) | Catalogue records the transfer. Users are told "has a new owner" before the next update installs (Q2) |
| Author abandons it | Stays listed with "no updates since <date>"; a fork is a new ID |

### 3. Versions and compatibility

A plugin declares, per app, the **contracts** it uses with major and minimum minor (recorded automatically by `craft-ext pack` from the inventories, T03), its style, and for native plugins its platforms. App version is an optional extra lower bound for workarounds, not the main rule (Q3).

| Check | Where | Outcome |
|---|---|---|
| Manifest schema | Install | Newer schema than the app knows: "Update the app to install this". Critical changes always bump the schema, so unknown critical fields cannot be skipped |
| Contract major/minor | Install and load | Same major and app minor ≥ required: compatible. Otherwise per contribution: unavailable with "needs <contract> <ver>; you have <ver>: update the app" or "this plugin is too old for this app" |
| Engine features and imports | Load (T06) | Refused with the feature named |
| Platform (native) | Install | Only a matching artifact is offered |
| Per app, per contribution | Setup | A multi-app package can work fully in one app and partly or not at all in another; each contribution shows its own reason |

The outcomes are: **compatible**, **partly compatible** (some contributions unavailable, each with its reason), **incompatible** (with what to update). Never a silent half-load.

### 4. Saved references in projects

A project stores, for each plugin feature: contribution ID, the exact version used, the contract version, parameters and the plugin's state with its schema version. On reopen the app uses the installed release if it is the same major and not older than saved; otherwise it shows a placeholder that keeps all data (T13). EffectCraft's bare `effect` id is migrated through `legacy_ids` declared in the new manifest; a package may only claim legacy IDs under its own namespace, so nobody can claim `ec.*` or another author's effects.

### 5. Existing systems

| Today | Becomes |
|---|---|
| craft-extensions `id`, `version`, files with SHA-256, provenance | Kept as is |
| `compat.min_app` + `interface` | Contracts per app with major/minor; `min_app` optional |
| EffectCraft/FigCraft `api` numbers | Legacy app profiles (T02) expressed as contract versions |
| EffectCraft project `Effect { effect }` | Contribution ID + version + state, through `legacy_ids` |
| Unknown fields ignored | Still ignored for optional data; critical changes bump `schema` |

## T08 session proposal: store and lifecycle

Status: proposed 2026-11-09 after an all-perspective review (user with several apps open, long export during an update, launcher and CLI writing at once, old and new app versions side by side, crash mid-install, low disk, support). Builds on T01 lifetimes, T05 grants, T06 native label, T07 identity. Not accepted.

### Evidence (craft-extensions `a6c5fae`, `crates/ext-store`)

Already good and kept: one shared folder under the user's local data, `CRAFT_EXT_DIR` override (`location.rs`); immutable `packages/<id>/<version>/`; unpack and check in `staging/` outside the lock; a short `.lock` for the move and the state write; atomic state replacement with a `generation` counter that apps poll; downgrade refused; first-party never replaced by a file; readers never write and treat unreadable data as absent (`store.rs:1-12`, `reader.rs`, `state.rs`).

Gaps: install **enables every app the manifest lists** (`store.rs:198-200`); update **deletes the old version folder immediately** after the state write, even if an app or export is using it (`store.rs:209-211`); records hold no digest, source or listing, grants or per-app choices other than enable and theme (`state.rs:23-44`); one schema number, so a newer state makes older apps see nothing.

### 1. Five kinds of state, each with one owner

| State | Lives in | Owner | Changes when |
|---|---|---|---|
| Release bytes | `packages/<id>/<version>-<digest>/`, never edited | Store | Installed; deleted only by cleanup |
| Package record | `state.json` | Store | Install, update, uninstall: source (store listing or local file), label, releases kept, default release |
| Per-app setup | `state.json`, keyed by app | User through any app, launcher or CLI | Enable per contribution, settings, theme choice, optional pinned release, grants (T05), "Allow more time" (T06) |
| Leases | `leases/<process>.json` | Each running app, CLI or job | Created when it loads a release, removed on exit; dead processes' leases are ignored |
| Caches | `cache/` | Any app | Validated or compiled module data; always safe to delete |

### 2. Transitions and what a crash leaves

| Transition | Steps | Crash at any step leaves |
|---|---|---|
| Install | Download from the author's release → check pinned digest → stage → validate → move into `packages` → write state. **Enables nothing** | Previous state; leftover staging or an unrecorded release folder is cleaned on next start |
| Setup in an app | Choose contributions, grant permissions | Previous setup |
| Update | Install new release **beside** the old one → state default = new → per-app choices carry over; new contributions start off; a widened permission or new owner keeps that app on the old release until the user accepts (T05/T07) | Old release still default |
| Running work during update | A running export or job keeps its lease on the old release and finishes with it; new work uses the new default (Q1) | — |
| Roll back | "Use previous version" pins the kept previous release for that app | — |
| Disable in an app | Setup off; grants kept | — |
| Uninstall | Record, setups and grants removed; bytes deleted once no lease uses them; projects show placeholders (T13) | Record still present |
| Cleanup | Deletes releases that are not default, pinned, previous-kept (Q2) or leased; caches first under low disk | Nothing lost: cleanup only deletes unrecorded or unreferenced data |

Low disk: an install checks free space for the archive and the unpacked size before downloading and fails with a clear message.

### 3. Several writers, several versions

- Writers (apps, launcher, CLI) keep the existing short lock and atomic replace with `generation`. An OS file lock is released automatically if a writer crashes.
- **Schema evolution uses two numbers, as SQLite's file header does**: `write_version` and `read_version`. Additive changes raise only `write_version`, so older apps can still read; an app refuses to *write* a state whose `write_version` is newer than it understands (it would drop fields) and asks the user to update it. Only a breaking change raises `read_version`.
- The one-time move from today's schema 1 keeps everything readable by apps not yet updated: what old apps understand (installed list, enable, theme) stays in place.

### 4. Updates

Checking for updates reads the catalogue (T09/T10). The store supports automatic and ask-first per package and per app; the default is Q3. Native plugins and new owners always ask (T06/T07).

## T09 session proposal: GitHub distribution and catalogue

Status: proposed 2026-11-09 after an all-perspective review (first-time author, regular author, maintainer, bot, end user browsing offline, impersonator, author who deletes their repo, attacker who takes over a renamed GitHub account). Starts from the catalogue design, adjusted to the settled decisions: store lists only (no building, hosting or responsibility), light review, MIT/Apache, native allowed, reverse-domain IDs (no UUID), always-ask updates, any listed version installable. Not accepted.

### Evidence

- craft-extensions `a6c5fae` `ext-core/src/index.rs`: an index with version, expiry, rollback refusal and revocations by `id` + `below`; unsigned; entries carry URL, size and apps.
- craft-launcher `c9c3aca` `crates/native/src/install.rs`, `http.rs`: GitHub Release download with resume, size cap and SHA-256; to be extracted for shared use (W6).
- EffectCraft `ce9eaec` `crates/engine/src/system/library.rs`: its own `index.json` in `storytold/effectcraft-plugins`, fetched unsigned with `curl`. To be folded into the one catalogue.

### The path

```text
author repo ── craft-ext prepare-release ──▶ GitHub Release on the AUTHOR's repo (zip; native: one zip per platform)
      │                                            │
      └── PR to catalogue repo: releases/<version>.toml
                                                   ▼
            bot (no secrets, read-only): download, digest, validate, licence, namespace, smoke test → report
                                                   ▼
            merge rule (Q1) ──▶ publish job: catalogue.json regenerated and signed (T10)
                                                   ▼
            apps / launcher / CLI: fetch catalogue (no login) → browse → choose version → download from author's
            GitHub → check pinned digest → store (T08) → setup per app (T05)
```

### Records (metadata only; the repository never holds plugin files)

| Record | Fields |
|---|---|
| Publisher | GitHub login **and numeric owner ID**, display name, official flag (maintainers only), status |
| Package | Package ID (T07), repository `owner/name` **and numeric repository ID**, licence (MIT and/or Apache-2.0), apps, summary, legacy IDs, native flag |
| Release | Version, tag and commit, per platform: URL, size, SHA-256; contracts per app; permissions; bot report link; channel |
| Revocation | Match by digest, package and version range, or publisher; severity `advisory`, `block-new`, `disable`; reason |
| Transfer | Old and new owner, date, maintainer; drives the "new owner" notice (T07) |

All approved releases stay listed so users can choose any version (T08 Q2).

### What the bot checks

Shared validator (same code as `craft-ext check` and the installer); pinned digest and size; licence file and SPDX expression MIT and/or Apache-2.0, bundled dependencies within the house allowlist; public source link at the tag; namespace ownership (T07); permission and contribution diff against the previous release; WebAssembly smoke test in the reference host. Native releases: static checks, and the smoke test only on an ephemeral runner with no secrets or write permissions. The bot never holds credentials that a plugin could steal.

### When GitHub changes underneath

| Event | Result |
|---|---|
| Release or repository deleted by the author | Installed copies keep working; that version shows "no longer available from the author"; nothing is mirrored |
| Asset replaced after review | Digest mismatch: the app refuses it; bot flags the listing |
| Repository renamed or moved | Numeric repository ID matches; bot updates the URL |
| Account deleted and the name re-registered by someone else | Numeric owner ID differs: listing frozen until a maintainer checks (blocks repo-jacking) |
| GitHub unreachable or offline | Browse from the last fetched catalogue; installs and updates wait; installed plugins unaffected |

### Author effort

One command (`craft-ext prepare-release`) writes the zip and the record; the template GitHub Action uploads the release; the author opens the catalogue PR. Immutable releases are recommended, not required, since the digest already pins the bytes.

## T10 session proposal: authenticity and incident operations

Status: proposed 2026-11-09 after an all-perspective review (attacker with an author's GitHub account, attacker with a maintainer's account, leaked signing key, malicious plugin discovered after thousands of installs, offline user, maintainer on call). Builds on T09 (bot auto-merge, pinned digests, numeric GitHub IDs) and the store's light-responsibility role. Not accepted.

### Evidence

- craft-extensions `a6c5fae` `ext-core/src/index.rs:1-6`: version, expiry and rollback refusal exist; signatures are "a later gate"; revocation only `id` + `below`.
- craft-launcher `c9c3aca`: the app list is built in (`crates/core/src/catalog.rs`); macOS builds checked against an Apple team ID; Flatpak OCI signatures for Linux. No signed metadata format exists to reuse.
- EffectCraft's plug-in index is unsigned and fetched with `curl`.

### Threats and the defence for each

| Threat | Defence |
|---|---|
| Catalogue changed by someone with repository or hosting access | Apps accept only signed metadata; the trust root ships inside the signed app; hosting gives no authority (Q1) |
| Author's GitHub account taken over, publishes a routine-looking update | The bot would auto-merge it (T09). Auto-merged updates are held before users see them (Q2); the author and watchers are notified by the catalogue PR; any report or maintainer can block during the hold |
| Release file swapped after the check | Pinned digest: refused |
| Old or frozen catalogue replayed to hide a revocation | Version and expiry already in the index; expired metadata is shown as stale and cannot authorize new installs |
| Online signing key leaked | Offline root keys (held by two or more maintainers on hardware keys) rotate it; apps follow without an app update (Q1 A) |
| Malicious plugin discovered | Revocation, below |

### Signing and key custody

- Online key: signs each publication and refreshes expiry; lives only in the protected publish job's environment, which requires a maintainer approval.
- Offline root: two of three maintainers must sign root changes; stored on hardware keys; recovery rehearsed once before launch.
- Operational owner: the catalogue maintainers, named when the repository is created (T09 Q2).

### Revocation

| Severity | Effect | Who can issue |
|---|---|---|
| Advisory | Warning on the listing and in the app | Any maintainer |
| Block new | No new installs, enables or updates to the matched releases | Any maintainer, immediately |
| Disable | Installed matched releases stop loading at the next check; running work stops at the next host call (native: process killed); an export in progress fails naming the reason (T06 Q3) | Any maintainer, immediately; reviewed by a second maintainer within a day |

Matches: exact digest (also applies to the same bytes installed from a file), package version range, or a whole publisher (account takeover). Apps check the small revocation list at start and every few hours when online; known revocations are kept across repair and restore. A revoked plugin's data and project references are never deleted; projects show a placeholder (T13).

### Incident steps (account takeover example)

1. Report arrives (catalogue issue form, or private security contact for sensitive reports).
2. Maintainer issues **block new** on the publisher immediately; **disable** if malicious code shipped.
3. Listing shows a banner; apps show a notice on next check.
4. Author regains the account; a recovery release gets a quick human look (as for first listings) before the block lifts.
5. Short public note in the catalogue repository.

### Offline

Installed plugins keep working offline with the last known revocations applied. Unknown revocations cannot be known; the next online check applies them.

## T11 session proposal: search and management UX

Status: proposed 2026-11-09 after an all-perspective review (new user looking for a glow effect, user of several apps, user offline, keyboard and screen-reader user, non-English user, author wanting visibility, maintainer, support). Builds on T05 (consent), T06 (labels), T07 (compatibility outcomes), T08 (lifecycle), T09/T10 (catalogue, holds, revocation). Not accepted.

### Evidence

- FigCraft `0e86f359`: a "Manage plugins" window with a Browse tab; Install first shows permissions, downloads are cancellable and the window stays responsive (`crates/ui-egui/src/plugins_browse_tests.rs:1-5`).
- EffectCraft `ce9eaec`: its own Plug-in Library in settings and dialogs (`crates/ui-egui/src/panels/settings.rs`, `dialogs.rs`).
- Each app builds its own manager; nothing is shared.

### 1. One manager component, shown in every app and the launcher

A shared egui component in craft-extensions (T03 A), using craft-libs `craft-i18n` for text. Inside an app it shows that app; the launcher shows all apps. The CLI offers the same actions (`<app>-cli plugin search|info|versions|install|setup|grant|update|remove`) through the same client.

### 2. Screens

| Screen | Content |
|---|---|
| **Browse** | Search box; filters: kind (effects, tools, panels, themes, resources), "works in this app", native; rows show name, summary, author, stars with date, labels (Official, Native: full access, Not reviewed) and a compatibility badge |
| **Detail** | Description, screenshots (pinned in the record, loaded on demand), what it can access in plain sentences (T05), per-app compatibility with reasons (T07), versions to choose from (T08), licence, source link, last updated, release notes |
| **Install and setup** | Install downloads (cancellable, shows progress) and then offers "Set up in <this app>": contributions on/off and permissions in one sheet. Other apps show "New plugin ready to set up" the next time they open |
| **Installed** | Per app: on/off per contribution, permissions with last use and revoke, version (choose another), update available, remove, problems |
| **Updates** | Badge with count; each update shows release notes and highlights more permissions, new owner and native. Always asks (T08 Q3) |
| **Problems** | Failed, paused, incompatible or blocked plugins, each with the reason and a fix button (update the app, choose a version, allow more time, remove); "Start in safe mode"; "Report plugin" link to the catalogue's form |

### 3. Ranking

Filter (query, this app, kind, compatible with this app and platform, not blocked) → relevance tier (exact name or ID, prefix, text) → official first **within** a tier (Q1) → repository stars → newer approval → ID for a stable order. Stars are fetched centrally by the publish job, never by desktops.

### 4. Built-in search suggestions (Q2)

When a built-in search (effects, commands) has no match, the app can show matching plugins from the **cached** catalogue: "No built-in match. 3 plugins in the store." The search never leaves the machine; a setting turns it off.

### 5. Offline, accessibility, language

- Offline: browse the cached catalogue with its date; installs wait; installed plugins work.
- Keyboard and screen readers: every action reachable by keyboard, labels and states exposed through the toolkit's accessibility support; labels never rely on colour alone.
- Language: app text through `craft-i18n`; plugin text in the author's language, with optional per-language fields in the release record.
- No account needed for anything public.

## T12 session proposal: plugin UI and interactions

Status: proposed 2026-11-10 after an all-perspective review (effect author who wants no UI code, panel author, tool author drawing on the canvas, user who relies on their shortcuts, keyboard and screen-reader user, user switching documents mid-gesture, theme author, CLI user). Builds on T01 (protected core, additive namespaces), T04 (one undo step per invocation), T05 (host-drawn prompts), T06 (worker thread, budgets), T08 (reload at safe points). Not accepted.

### Evidence

- House rule (Rust repositories, rule 4): native Craft apps have no webview, Tauri or Electron, and no handwritten JavaScript. Plugin UI therefore cannot be HTML.
- EffectCraft `ce9eaec`: effect parameters are declared as data (`PluginParamKind`: slider, angle, point, 3D point and more) and the host builds the controls (`crates/effects/src/plugin.rs:55-110`).
- FigCraft `0e86f359`: panels and settings as data with state kept between callbacks and one undo step per editing callback (API 2); canvas widgets drawn from plugin descriptions, clicks and hover as one undo step each, kept through undo, save and reload; junk descriptions, runaway callbacks and forged widget data refused (API 3) (`crates/engine/src/plugin_panels_tests.rs:1-4`, `widgets_tests.rs:1-4`).
- craft-libs: `craft-keymap` already has default bindings, user overrides, conflicts, reserved system chords and a shortcut editor model; `craft-menu` builds menus from `CommandRef`s; `craft-ui` has docking, panes, focus, IME and themes.

### 1. Host draws everything; plugins describe

Plugins return data; the app draws it with its own widgets. Consequences: plugin UI matches the app and its theme, is accessible and localized through the same toolkit, cannot imitate a host permission prompt, and a slow plugin never freezes the UI because nothing runs on the UI thread. This is the approach of Blender's layout API, Slack Block Kit and Adaptive Cards.

### 2. UI contribution kinds

| Kind | What the author writes | Host behaviour |
|---|---|---|
| Effect parameters | `#[derive(craft::Params)]` only | Controls, keyframes, expressions and presets come from the host. Most effect authors write no UI code |
| Commands | A command contribution | Appears in menus, command search, CLI and MCP (T04) |
| Menu items | Placement in a declared extension point (Q3) | Added after the built-ins, grouped under the plugin; never removes or reorders built-ins |
| Shortcuts | Optional suggested default (Q2) | Through `craft-keymap`; user can rebind anything |
| Panels and dialogs | A tree: rows, columns, labels, buttons, fields, sliders, lists, tables, images from the package, progress, plus a **custom canvas** element (Q1) | Docked with `craft-ui`; plugin dialogs carry the plugin's name and icon so they cannot pass as the app |
| Inspector and context-menu sections, settings pages | Same tree | Shown in the host's places |
| Canvas overlays and widgets | Draw lists (shapes, paths, text, images) in document coordinates, bounded in count | Host renders and hit-tests; hover and click arrive as events |
| Pointer tools | begin, update, commit, cancel | Host captures input; updates are coalesced to the latest so the pointer stays smooth; preview through an overlay; commit is one undo step; Escape or losing capture cancels with the document unchanged |

### 3. Events, lifecycle and undo

- Every UI event is an invocation (T04) on the plugin's worker thread under the command budget (T06). While it runs, the control shows busy; the rest of the app works.
- Open creates the session instance; close disposes it and drops its callbacks. Switching document sends a scope change; a gesture in progress is cancelled first.
- Update or disable closes the plugin's UI with a notice and reopens it at the next safe point (T08).
- Edits from a callback or gesture are one undo step labelled with the plugin. UI-only state is not in undo.
- Where things are kept: panel placement by the host per app; plugin settings in its private storage per app; widget data in the document (as FigCraft does).
- Headless (CLI, MCP, render): UI contributions do not exist; commands remain available.

### 4. Accessibility, language, themes

The SDK refuses a control without a label at build time. Text uses the package's localization keys with the host choosing the language. Themes apply automatically because the host draws.

## Existing app plugin systems are replaced (2026-11-10)

User direction: the plugin systems apps have today (EffectCraft WASM ABI v1/v2, helpers and Plug-in Library; FigCraft APIs 1 to 4; PhotoCraft filter ABI) will be removed completely and replaced by this design. Wherever earlier sections cite them, they are lessons, not compatibility constraints. Superseded: T02 legacy app profiles, the T05 migration table, T07 `legacy_ids`, folding EffectCraft's index into the catalogue (T09). Old projects referencing removed plugins open with placeholders that keep their data (T13).

## T13 session proposal: live projects and reliable output

Status: proposed 2026-11-10 after an all-perspective review (artist reopening last month's project, freelancer sending a project to a client without the plugin, studio render farm, two collaborators, plugin author changing their saved-state format, user offline before a deadline). Builds on T06 Q3 (failure fails export), T07 (saved references), T08 (current release only, any version downloadable), T10 (revocation), and the replacement of existing plugin systems. Not accepted.

### Lesson from today's code

EffectCraft `ce9eaec` `crates/render/src/pipeline.rs:622-632` drops any effect whose ID it cannot find (`filter_map` on `find(effect)`), so a project with a missing plugin renders and exports "successfully" without it. The new design must make that impossible.

### 1. What a project stores per plugin feature

Contribution ID, exact version used, contract version, parameters, the plugin's own state with its schema number (T07), and a **plugin list** at the project level (ID, version, catalogue link or "local only") so the app can tell what is needed before loading anything. Grants are never stored (T05).

### 2. Reopening

| Situation | Result |
|---|---|
| Same or newer release, same major, installed and set up | Used. If the version differs: a quiet note "Made with Grain 1.2, using 1.4" (Q1) |
| Saved state schema older | Plugin's `migrate(old_schema, data)` runs; the result is saved only when the user saves. Migration failure → placeholder |
| Installed release older than saved, or different major without declared migration | Placeholder: "Needs Grain 1.2 or newer" with Update |
| Not installed / not set up in this app | Placeholder with **Install** or **Set up** (one prompt lists all missing plugins: "This project uses 3 plugins") |
| Revoked | Placeholder with the revocation reason; data kept |
| Plugin from a removed old system | Placeholder; data kept |

**A placeholder keeps everything**: parameters, state, keyframes, position in the stack. Preview passes the input through with a badge; editing everything else works; saving writes the plugin's data back byte for byte. Nothing is ever silently dropped.

### 3. Output

- **Final render and export** with any placeholder, failed or blocked plugin **fails before starting**, listing the plugins and how to fix each (Install, Update, Set up). Same in the CLI and render queue, with the exact `plugin install` commands.
- **Preview** may use draft quality and scale; final render always runs at final quality. Plugins receive a `quality` hint; caches key on it, so a preview frame is never reused as final.
- **Caches** key on release digest, contract version, parameters, inputs and quality. An update or version change invalidates exactly that plugin's results. Determinism (T02) makes caches valid across machines.

### 4. Portability

- **Bake (Q2)**: "Bake plugin result" renders a plugin feature to pixels, frames, geometry or values as the app's own data, so the project opens anywhere. The original plugin feature is kept, switched off, so it can be un-baked. This is After Effects pre-render, Photoshop rasterize and a DAW's freeze.
- **Projects do not embed plugin files (Q3).** The plugin list carries catalogue links; the receiver installs through the store or a file with their own consent.
- **Collaboration**: plugin edits go through the same operations (T04), so they merge, sync and undo like built-in edits. Plugin state must be plain serializable data, never memory addresses or machine paths (files are tokens, T05).

### T13 outcome (2026-11-10)

Q1 A and Q2 A as proposed. Q3 changed to **B, off by default**: an explicit "include plugins" option when saving or collecting files puts the plugin archives in the project. On open they are listed, never installed or run automatically. "Install from project" is an ordinary file install through the store's validation; an archive whose digest matches a catalogue listing is treated as that listing, otherwise it is "Not reviewed" (native ones also "full access"); revocation applies by digest; setup and permissions as usual.

## T14 session proposal: typed resources, themes and workflows

Status: proposed 2026-11-10 after an all-perspective review (colourist installing a LUT pack, theme author, user with a broken theme, person installing a 3 GB model, workflow author, user running a downloaded workflow, translator, user with a hostile file). Builds on T02 (data style, typed resource contracts), T05, T08, T13. Not accepted.

### Evidence (platform code to keep, not app plugin systems)

- craft-extensions `a6c5fae`: `Kind::Theme` and `Kind::Resource`; resources capped at 512 files, 64 MiB each, 256 MiB total (`manifest.rs:44`); the shared theme is a small palette every app maps onto its tokens plus optional per-app overrides, and a theme that breaks WCAG 4.5:1 contrast cannot be installed (`palette.rs:1-6`).
- craft-libs `craft-keymap` has named shortcut profiles and imports; `craft-ai-core` is the interim shared model store contract.
- GitHub release assets are limited to 2 GiB each.

### 1. One rule for every data kind

Data packages contain no code. Each kind has one **importer** that validates hostile input with size, count, dimension and nesting limits; installation never runs anything; a file that fails validation is refused with the reason. Shared kinds have one importer in craft-extensions or craft-libs used by every app; app-only kinds have the app's importer.

| Kind | Importer | Notes |
|---|---|---|
| Themes | Shared palette + per-app overrides (exists) | Never auto-activated; invalid → app keeps its own look; contrast enforced; scope Q3 |
| Effect and tool presets | App | Refer to a contribution ID + parameters; a preset for a missing plugin shows Install |
| LUTs, ICC profiles, gradients, swatches, patterns | Shared | Typed resource contracts from T02 |
| Brushes, shape and symbol libraries, templates | App | Templates are documents opened through the app's normal loader with its limits |
| Fonts | Shared font store | Licence from the house allowlist (OFL, etc.); usable inside Craft apps, not installed system-wide |
| Keymap profiles | `craft-keymap` profiles | Applied only when the user chooses; cannot bind management or hidden commands |
| Localization packs | `craft-i18n` | Translate app text, but **never** permission, consent or warning text, which always comes from the app's own translations, so a pack cannot make a dangerous plugin sound harmless |
| ML models | `craft-ai-core` store | Large; see section 2 |
| Workflows | App or shared graph contract | Section 3 |

### 2. Large resources

A package may reference large files (models, big template libraries) by URL, size and SHA-256 instead of carrying them (GitHub's 2 GiB asset limit; avoids huge installs). States: **not downloaded, downloading, ready, failed**. Download happens on first use with the size shown (Q2), is resumable, checked against the digest, and stored content-addressed so two packages sharing a model keep one copy. Removed when no installed package refers to it. Offline before download: the feature says it needs a download.

### 3. Workflows

A workflow is a graph of operations (T04) and plugin contributions with typed inputs and outputs. It can be inspected without running: the app lists the operations, plugins and permissions it needs. Running is a job (T15) with progress and cancel. **Authority (Q1)**: it runs as its own package with the permissions it declares, approved at setup like a plugin, so a "data-only" download cannot use the user's full power just because the user pressed Run. Missing nodes become placeholders (T13); the workflow cannot run until they are installed.

## T15 session proposal: automation and jobs

Status: proposed 2026-11-10 after an all-perspective review (script author, AI agent over MCP, user with the app open while an agent edits, render-farm operator, another local program or web page trying to reach the app, another account on a shared machine, plugin). Builds on T04 (one operation path, automation authority Q1 A, management scope), T05 (CLI grant commands), T06 (budgets), T13 (export fails on missing plugins). Not accepted.

### Evidence

- EffectCraft `ce9eaec`: the control server listens on TCP `127.0.0.1:9877` with no token (`apps/effectcraft/src/control_server.rs:27-34`; a test sends a request with a foreign `Origin`, line 183). MCP long renders run as a background job with `notifications/progress` and stop on `notifications/cancelled`, deleting partial output (`crates/automation/src/long_job.rs:1-6`).
- FigCraft `0e86f359`: the MCP backend attaches to the running app over plain TCP (`crates/mcp/src/backend.rs:96-123`).
- PhotoCraft `655295b2`: per-command `authorize_*` for automation (`crates/automation/src/workspace.rs:327-398`).

Any local program, any other account on the machine, and in some cases a web page can reach today's open ports.

### 1. Three environments, one operation set

| Environment | What it is | Plugins |
|---|---|---|
| Fresh CLI | A headless process opening a project | Installed releases set up for that app, same eligibility as the GUI; never prompts |
| Attached | Controls the running app | Whatever the app has loaded |
| Server | A long-running headless app (render node) | As fresh CLI |

All three call the same operations (T04) with the same results and error codes. Existing CLI commands stay as aliases of generated operations. A fresh CLI refuses to write a project that is open in a running app (per-document lock file) and suggests `--attach`, so an agent cannot overwrite unsaved work.

### 2. Attaching safely (Q1)

- Transport: a Unix domain socket in a user-only directory (macOS, Linux) or a named pipe restricted to the user (Windows), plus a random per-session token in a user-only file. Browsers cannot open these, and other accounts cannot reach them. This is how Docker, VS Code's CLI and language tools attach. No network port by default; remote server mode is a later explicit configuration (TLS and token).
- Authority: the user's document authority; management operations only after in-app approval once per session (T04 Q1).
- While an external client is connected, the app shows "**<client> is controlling EffectCraft** · Stop" (Q2).
- Several clients may attach; each is its own principal; edits are serialized by the dispatcher.
- Plugins have no sockets and never receive the token (T04).

### 3. Jobs

One job model in every environment: ID, owner, state (queued, running, succeeded, failed, cancelled), progress, outputs, structured error.

| Client | Progress | Cancel | Disconnect or exit |
|---|---|---|---|
| CLI | `--json` progress lines; human progress otherwise | Ctrl-C cancels cleanly | Fresh: the job ends with the process |
| MCP | `notifications/progress` | `notifications/cancelled` | Attached: Q3 |
| Attached | `jobs list`, `jobs wait <id>` | `jobs cancel <id>` | Q3 |

App quit with running jobs: the GUI asks; headless on SIGTERM cancels cleanly and exits with the cancelled code.

### 4. Outputs and errors

- Outputs are written to a temporary name and renamed when complete, so an incomplete file never looks finished; cancelled or failed jobs delete partial output. Results list paths, sizes and digests.
- Stable exit codes shared with MCP error data: 0 succeeded, 1 failed, 2 usage, 3 input required (lists what is missing), 4 plugin missing or not permitted (prints the `plugin install` or `plugin grant` command), 5 cancelled.

## T16 session proposal: performance, offline behaviour and diagnostics

Status: proposed 2026-11-10 after an all-perspective review (artist scrubbing a 4K timeline with three plugin effects, export on a laptop with 8 GB, user on a plane, plugin author chasing a slowdown, support helping a user whose app will not start, privacy-minded user). Builds on T06 (budgets, profiles, wasmi), T08, T10, T13, T14, T15. Not accepted.

### Evidence

- The workload profiles draft: interactive, preview, background/export and real-time audio, with per-instance and app-wide accounting and explicit raises.
- EffectCraft `ce9eaec` has no safe-start mode; no shared log rotation or support bundle was found in the audited apps.

### 1. Budget numbers come from measurement

- `craft-ext-conformance` gains a **benchmark set**: reference plugins (blur, halftone, LUT apply, a panel callback, a geometry edit) run on named reference hardware in CI.
- Default budgets = measured cost of the reference workload × generous headroom, per profile (T06). Raised per plugin by the user ("Allow more time"), never beyond the host's maximum.
- App-wide: plugin memory has a total ceiling as a share of system memory; parallel instances reduce to fit (PhotoCraft's band rule). Interactive and preview work outranks background export in the shared scheduler so a long export never makes scrubbing stutter.
- **Engine re-evaluation trigger** for T06 Q1: when a reference plugin effect at 4K is several times slower than the equivalent built-in and drops preview below real time on reference hardware, evaluate a compiling engine on desktop behind the same host trait.

### 2. Making it fast

Validated modules cached per release digest; instances reused within a job (T06); tiles or bands in parallel; buffer copies counted in the budget and minimized by the contract's buffer handles (T02); preview and final caches keyed separately (T13); deterministic output lets caches be reused across machines.

### 3. Seeing performance (Q1)

The Installed screen shows each plugin's recent cost ("used 38% of the last render", "slowest frame 210 ms") and memory. An optional overlay in the viewer shows per-plugin time for the current frame. Authors get the same numbers in `craft-ext dev`.

### 4. Offline, in one place

| Works offline | Needs a connection |
|---|---|
| Installed, set-up plugins; projects; placeholders; baking | Installing, updating, choosing another version |
| Browsing the cached catalogue (dated) | Fresh revocations (cached ones still apply) |
| Plugins' own private storage | Large resources not yet downloaded (T14) |
| No sign-in is ever required for public plugins | Plugins using network permission get a clear "offline" error they can handle |

### 5. Logs and support

- Per-app local log, size-capped and rotated; plugin lines marked with the plugin ID and treated as untrusted text; paths, URLs and document content redacted by default.
- Plugin authors log through the SDK; output appears in a **Plugin console** panel and in `craft-ext dev`.
- **Create support report** (explicit action): logs, installed plugin list with versions and digests, store state without secrets, app and OS version. The user sees the contents before saving; nothing is uploaded automatically and there is no crash telemetry.

### 6. Safe start (Q2)

- The app marks "loading plugins" at start and clears it once running. If the previous start did not clear it, the next start asks: "EffectCraft closed while loading plugins. Start with plugins off?"
- Also on demand: `--safe-mode`, and Help ▸ Restart with plugins off.
- Safe mode runs no plugin code and applies no plugin themes; the manager works for disabling or removing; projects open with placeholders and save their plugin data unchanged.
- "Find the problem plugin" (as VS Code's extension bisect): turns half the plugins off per restart until the culprit is found, then restores the rest.

## T17 session proposal: ecosystem policy and long-term maintenance

Status: proposed 2026-11-10 after an all-perspective review (author refused a listing, author whose plugin broke on a new contract major, maintainer team of a few people, app maintainer wanting to change a shared contract, user of an abandoned plugin, bosses asking what this costs). Consolidates policy already settled and adds what is left. Not accepted.

### 1. Policy already settled

| Area | Settled |
|---|---|
| Store role | Lists only; binaries from authors' GitHub Releases; no building, hosting or responsibility (store role) |
| Licences | Store plugins MIT and/or Apache-2.0, bundled code and assets from the house allowlist; local installs any licence, labelled (T05) |
| Native | Allowed, labelled "Native: full access to your computer" (T06) |
| Checks | Bot always; quick human look for first listing, more permissions, new owner, native; 48-hour hold for auto-merged updates (T09, T10) |
| Official plugins | Same catalogue, same rules and licences, official flag set by maintainers only; rank first within a relevance tier (T11) |
| Proprietary or paid distribution | Not a store goal; no credential or entitlement integration |

### 2. Listing rules (Q2)

Refused or removed only for: malicious or deceptive behaviour; impersonation (names, icons or descriptions implying Storytold endorsement, reserved prefixes); illegal content; failing the bot checks; unverified namespace. Never refused for quality, for competing with a built-in feature, or for being native. Published as `policy/listing-rules.md` in the catalogue repository.

### 3. Decisions and appeals (Q3)

A refusal or block states the rule and evidence in the catalogue PR or issue. The author may appeal in the catalogue repository; a different maintainer decides, normally within 14 days. Emergency blocks stand during the appeal.

### 4. Contract support and deprecation (Q1)

- A contract major stays supported for a fixed period after its successor ships; apps keep both adapters during the overlap. Security-critical breaks may end support early with a published reason.
- Deprecations are announced in the generated contract changelog and shown to authors by `craft-ext check` as warnings; users see "this plugin uses an old interface and will stop working in <app> after <date>" only in the final months.
- Contract changes go through a PR in craft-extensions; a shared domain contract change needs approval from maintainers of at least two apps that use it (T02 governance).

### 5. Ownership

| Part | Owner |
|---|---|
| craft-extensions crates, SDK, `craft-ext`, conformance and benchmarks | Platform maintainers |
| Each app's adapter, exported contract and its CI check | That app's maintainers |
| Catalogue repository, bot, keys, incidents | Catalogue maintainers (named when created, T09) |
| Shared domain contracts | Platform maintainers with the consuming apps' maintainers |

Names are for the bosses to assign; the design only requires that each part has one.

### 6. Documentation

Generated from the inventories: contract reference (docs.rs once crates are published, GitHub Pages until then), the contract changelog and compatibility tables. Hand-written: an author guide, one template per contribution kind (`craft-ext create`), small example plugins, the listing rules and the incident process.

### 7. Cost and scope

No servers: GitHub repositories, Actions and Pages. Ongoing cost is maintainer time (quick looks, incidents, appeals) and CI minutes (bot, benchmarks, conformance). Bot auto-merge keeps routine updates free of human time.

## T18 session proposal: delivery and evidence

Status: proposed 2026-11-10. Orders the work that T01–T17 decided, without starting code. Replaces the draft implementation order where it assumed reusing existing app ABIs and a PhotoCraft-first slice, both superseded by the replacement direction. Not accepted.

### Principles

- Platform pieces first, each small, tested and shippable on its own; then one app at a time.
- An app never has two plugin systems: its old system is removed in the same change set that lands its new adapter (Q2).
- No unit starts without its owner named (T17) and its part of the merge hold lifted.

### Units in order

| # | Unit | Where | Depends on | Evidence before moving on |
|---|---|---|---|---|
| P0 | Accept the decisions, name owners, plan hold lifting per area | Maintainers | — | Owners named; this brief accepted |
| P1 | Store foundation: install enables nothing, per-app setup, grants record, leases, digest and source, read/write version numbers, user-chosen version change, manifest schema 2 (IDs, per-app contracts, permission families) | craft-ext-core, craft-ext-store | P0 | Crash at every step, concurrent writers, old reader still reads, update during a leased job |
| P2 | Operations kit: `Invocation`, principals, descriptor fields, `authorize`, errors, job model, exit codes | New craft-extensions crate | P0 | Unit tests incl. denial before side effect, origin chain, management never exposed to plugins |
| P3 | Host runtime and hostile suite: one wasmi profile (PhotoCraft pattern), budgets, instance rules, failure contract | craft-ext-host, craft-ext-conformance | P0 | Hostile suite passes on every desktop target and web |
| P4 | First contract and SDK: inventory format and generator, `raster.filter@1`, `craft-guest`, macros, `craft-ext` create/check/test/pack/prepare-release/dev, reference host, example halftone | craft-extensions | P2, P3 | Example plugin built, checked, tested and packed deterministically twice with the same digest |
| P5 | Shared manager component and `plugin` CLI commands (local file installs) | craft-extensions | P1 | Install, setup, grant, version change, remove driven headless |
| P6 | **EffectCraft pilot**: dispatcher migrated to `Invocation` (caller, kinds, management split); attach over user-only socket and token; raster adapter with temporal basics; manager; placeholders and export refusal; **old plugin crate, Plug-in Library and helpers removed** | EffectCraft | P1–P5 | GUI/CLI/MCP parity; export refuses a missing plugin; update during export; halftone runs; no open TCP port |
| P7 | Catalogue: repository (when maintainers create it), bot, TUF signing, 48-hour hold, revocation, client fetch using the download routine extracted from the launcher | New repository, craft-ext-store | P1, P4 | Swapped asset refused; expired metadata refused; revocation applied offline after one sync |
| P8 | **FigCraft**: session style (exposed operations, panels, tools, canvas, consent UI); APIs 1–4 removed | FigCraft | P6 | Panel/CLI parity; denied file operation; Escape cancels a tool unchanged; one undo step per edit |
| P9 | **PhotoCraft**: the same halftone module runs unchanged; old filter ABI removed | PhotoCraft | P6 | Identical output to EffectCraft for the same input (T02 promise) |
| P10 | Native plugins: out-of-process helper protocol and full-access label | craft-ext-host, EffectCraft first | P6 | A native effect crashes without affecting the app; revocation stops the process |
| P11 | Other apps by demand, each through the adapter checklist; FilmCraft temporal next; themes revisit | Each app | P6–P9 | Per-app proofs from the work packages |
| P12 | Public launch: catalogue live with keys held, listing rules, author guide, templates, examples, benchmarks setting budgets | All | P7, P8 | Community enablement decided by maintainers |

P1, P2 and P3 can run in parallel. P7 can run alongside P6 to P9; until it exists, pilots use labelled local file installs (Q3).

### Evidence still missing (verify during the units, not before)

wasmi 2's accepted WebAssembly features for the SDK template (P3/P4); a maintained TUF Rust client and its licence (P7); budget numbers from benchmarks (P12); egui accessibility coverage for the manager and declarative UI (P5/P8); which app maintainers own each adapter (P0).

### Merge hold

Each area of `docs/plugins/merge-hold-list.md` lifts when its unit lands; the EffectCraft plugin areas lift at P6 because P6 replaces them.

## App adapters and standalone management (2026-11-10)

User direction: full plugin management must be available inside every app, so users never need the launcher, in launcher-installed and standalone installs and builds alike.

### What an adapter is

One **app adapter per app** (not per plugin), in the app's repository. Every plugin used in that app goes through it. It connects the shared platform to that app and contains no copy of shared logic:

| Adapter part | Uses |
|---|---|
| Contract export (`xtask export-contract`) and its CI check | T03 |
| Implementations of the shared domain contracts the app offers (raster, temporal, …) and their conformance run | T02 |
| Operation classification: kind, exposure, `requires`, management and core markers | T04 |
| UI extension points: menu spots, panel docking, inspector sections, tool slots, canvas overlay | T12 |
| Project integration: saved references, placeholders, export refusal, bake | T13 |
| Jobs: render or export queue integration | T15 |
| **Management**: embeds the shared manager component (Settings ▸ Extensions and an Extensions menu item) and wires the shared `plugin` CLI commands | T11, T05 |
| Headless plugin-test entry for conformance | T03 |

### Management without the launcher

- Each app links the same craft-extensions crates the launcher uses: store, catalogue client, download routine, manager component. Browse, install, set up, grant, version change, update and remove all work in the app and its CLI. No launcher, daemon or account is needed.
- **One store for both**: launcher and apps use the same shared folder (existing `location.rs` rule, `CRAFT_EXT_DIR` override), so something installed in the app shows in the launcher and the other way round. The store lock and generation counter (T08) handle both being open at once.
- The launcher adds only an all-apps view and deep links into an app's manager. Nothing is launcher-only.

### Builds and packaging

| Build | Store |
|---|---|
| Standalone download (macOS, Windows, Linux tarball) | Shared folder |
| Launcher-installed | Shared folder |
| Linux Flatpak (the launcher installs per-user Flatpaks, `craft-launcher` `crates/native/src/packages.rs:16`) | The sandbox hides the shared folder by default. The Flatpak manifest grants access to that one folder only (`--filesystem=xdg-data/<store folder>:create`); if a distribution refuses that permission, the app uses its own store and the manager still works fully, without sharing |
| Web build | Browser-local store per app; install from file works; catalogue installs only if GitHub release downloads are reachable from a browser (cross-origin rules unverified) |
| Builds made without network code (offline products) | Manager with local file installs only, labelled; an advertised build choice, not another state format |

## Appendix: per-app work packages

All rows inherit discovery/compatibility/activation/loading, caller policy, transactions, persistence, jobs, CLI/artifacts, errors, themes where supported and conformance fixtures from the full adapter checklist. “Gap” means an existing source-derived concern or an integration not established by the survey. “Design” names additional functionality to support, not functionality already implemented. Concrete code edits require the operation audit above.

| App | Expose through app-owned contracts | Known gap or required integration | App-specific proof |
|---|---|---|---|
| EffectCraft | Composition/layer/selection queries, keyframes and effect parameters, effect evaluation, frame dependencies, render jobs and artifacts | Fresh CLI loader differs from desktop; missing effects can be omitted; process registration/update and pooled state need explicit lifecycle; preserve existing WASM/helper semantics | Independent layer states, seek/reset, update during export, cancelled render, missing required effect prevents success |
| FigCraft | Node/selection queries, geometry/layout edits, components, commands/panels/tools/widgets, file artifacts | Headless clears offered-file UI requests; disk modules need integrity continuity; same-ID replacement/settings and permissions need integration | Same native result through menu/panel/CLI; denied nested/file operation; undo/stale handles/panel close |
| PhotoCraft | Pixel/layer/mask/selection operations, filters, smart features, colour-aware data, import/export and themes | Bridge existing band/filter ABI and theme branch; failed/missing smart filter can be skipped; fresh CLI and exact smart-feature references need work | Alpha/colour modes, overlap seams, selected regions, smart save/reopen, theme fallback and explicit output failure |
| FilmCraft | Timeline/clip/track/selection edits, transitions, media inspection, export jobs; later frame/audio processing | No processing plugin host established by sampled sources; design temporal interface and independent theme mapping; preserve existing job completion behavior | Timebase/frame dependencies, timeline undo, awaited export, playhead/selection/highlight/contrast preserved |
| VectorCraft | Object/geometry queries and edits, units, live appearances, format/export handlers and resources | Preserve distinct object/live-effect/export contracts; connect release/state/retention and cached appearance evaluation | Malformed geometry rejected; units preserved; live effect save/reopen; output artifact completed |
| SoundCraft | Routing/project edits, prepare/process/reset, instruments/MIDI/automation, latency/tail, preset/state and editor lifecycle | Existing native formats are not sandbox proof; dedicated real-time backend and worker-failure semantics required | Deadline/latency/tail evidence, transport discontinuity, state restore, worker failure without unsafe callback behavior |
| LightCraft | Catalog queries and scoped metadata/develop edits, RAW/develop context, presets and export jobs | Define catalogue access scopes and recipe/live references; protected DB access must remain behind app operations | Nondestructive reopen, missing source media, export cancellation and selected-item scope enforcement |
| PDFCraft | Page/object queries and edits, annotations/forms where app supports them, validation/preflight and output | Survey establishes facade/tool table, not a plugin object model; add scoped handles and transactional file/output path | Stale page/object rejection, history, preflight result and completed PDF write |
| DesignCraft | Stories/frames/pages/spreads/styles, layout/reflow, pointer tools, prepress and export | Move UI-bound behavior into engine where audit proves duplication; define gesture/reflow transactions | Cancelled gesture, reflow undo, units, invalid layout rejection and prepress artifact |
| CADCraft | Entity/model queries, units/constraints where supported, explicit-parameter commands and tools | Interactive prompts need headless parameter contracts or explicit UI-only designation | Missing prompt returns InputRequired, unit-correct edit, rollback and model save |
| SolidCraft | Candidate model/entity commands, feature/rebuild operations, geometry and saved procedural state | Existing manifest listing does not establish executable host; implement actual runtime/transaction path before claiming support | Real command invocation, validated model mutation, missing procedural feature preserved and failed rebuild reported |
| TextureCraft | Mesh/UV/texture-set/channel queries and edits, brushes/strokes, material/bake/composite jobs | Resource/mesh lifetime, stroke transaction and cache ownership need adapter integration | Wrong texture-set rejected, cancel stroke/bake, update during lazy resource use, UV invalidation |
| WordCraft | Durable text ranges, stories/styles, structure/layout edits, validators and output | Define positions that survive or explicitly invalidate across edits; no raw persistent string offsets | Concurrent insertion/stale-range handling, undo, layout and exported document |
| GridCraft | Typed range/cell/workbook operations, import/export; dedicated formula/function extension contract | Custom functions need recalculation graph, cycles/error propagation, deterministic evaluation and budgets | Range edit undo, cycle/timeout, type errors, recalc invalidation and workbook save |
| DeckCraft | Slides/shapes/text, transitions, assets, presentation/export jobs and optional playback callbacks | Playback subscriptions have a different lifetime from commands; connect saved state/output contracts | Shape save/reopen/export, stop callback disposal, stale slide target and cancelled export |
| ArtCraft | Media/model/provider jobs, input/output roles, progress/cancel/result ingestion and workflow resources | Native target/provider jobs established, but plugin host/common CLI not established; egress/credential scopes needed | Provider restart/cancel, no credential disclosure, remote/offline readiness and durable result ingestion |

Candidate operations do not authorize exposing every internal action. App maintainers choose the supported contract, and denied side effects remain denied even when a plugin invokes a compound operation through parameters.
