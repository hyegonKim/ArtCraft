# Craft plugin architecture

Status: **proposal for review, 2026-11-10.** This describes the plugin and add-on system for the Craft apps that this repository would host. It was settled in a series of design sessions; each choice and its wording is in [DESIGN-DECISIONS.md](DESIGN-DECISIONS.md), and each topic's reasoning, evidence and rejected alternatives are in [TOPIC-PROPOSALS.md](TOPIC-PROPOSALS.md). How it compares with the code on `main` today, and where it disagrees with [THREAT-MODEL.md](THREAT-MODEL.md) and the earlier handover decisions, is in [ARCHITECTURE-GAP-ANALYSIS.md](ARCHITECTURE-GAP-ANALYSIS.md).

This is a design. Nothing here is accepted by the maintainers yet, and it does not authorize implementation, new repositories, publication or releases. Each delivery unit (section 22) needs its own approval and pull request.

## Contents

1. [Summary](#1-summary)
2. [Goals and principles](#2-goals-and-principles)
3. [Vocabulary and lifetimes](#3-vocabulary-and-lifetimes)
4. [What plugins can and cannot do](#4-what-plugins-can-and-cannot-do)
5. [System map](#5-system-map)
6. [Contracts](#6-contracts)
7. [One operation path](#7-one-operation-path)
8. [Permissions and consent](#8-permissions-and-consent)
9. [Runtime and safety](#9-runtime-and-safety)
10. [Identity and compatibility](#10-identity-and-compatibility)
11. [Store and lifecycle](#11-store-and-lifecycle)
12. [Distribution and catalogue](#12-distribution-and-catalogue)
13. [Authenticity and incidents](#13-authenticity-and-incidents)
14. [Management in every app and the launcher](#14-management-in-every-app-and-the-launcher)
15. [Plugin UI](#15-plugin-ui)
16. [Projects and reliable output](#16-projects-and-reliable-output)
17. [Resources, themes and workflows](#17-resources-themes-and-workflows)
18. [Automation and jobs](#18-automation-and-jobs)
19. [Performance, offline and diagnostics](#19-performance-offline-and-diagnostics)
20. [SDK and author experience](#20-sdk-and-author-experience)
21. [App adapters](#21-app-adapters)
22. [Delivery plan](#22-delivery-plan)
23. [Policy and maintenance](#23-policy-and-maintenance)
24. [Open items](#24-open-items)
25. [What this replaces](#25-what-this-replaces)

## 1. Summary

- **An open ecosystem.** Anyone can build plugins. A plugin can call every operation the app's CLI and MCP tools offer, can add effects, tools, panels, commands, formats, themes and resources, and can be native code. A plugin can use built-in features but cannot change, replace or remove them.
- **Safe by design.** WebAssembly plugins run in a sandbox (wasmi). They reach data only through permissions the user grants per app. Native plugins are allowed and carry a "Native: full access to your computer" label.
- **One path for every caller.** Menus, shortcuts, panels, the CLI, MCP agents, scripts and plugins all go through one `execute(invocation, operation, arguments)` path. It checks who is asking, runs one transaction and records one labelled undo step.
- **A store that lists, not hosts.** Plugin files stay on the author's GitHub Releases. A metadata-only catalogue pins the exact file hash it checked. A bot runs checks, and a human takes a quick look for risky cases. The project builds nothing, hosts nothing and takes no responsibility for plugins. Store plugins are open source under MIT and/or Apache-2.0.
- **Management everywhere.** Every app has the full manager built in, and so does its CLI. The launcher is optional and shares the same store.
- **Reliable output.** Projects record exactly which plugins they use. A missing plugin becomes a placeholder that keeps all its data, and a final export refuses to run without its plugins.
- **One platform, one repository.** Every shared crate lives in `storytold/craft-extensions`. Each app has one adapter in its own repository. The apps' current plugin systems are removed and replaced, app by app.

## 2. Goals and principles

| Principle | Meaning |
|---|---|
| Do not restrict what the community builds | The full CLI operation surface is available; native plugins are allowed; listings are never refused for quality or for competing with built-ins |
| Protect the core | Built-in features, defaults, shortcuts, management and updates cannot be changed by plugins (section 4) |
| Protect users by design, not by review | Sandbox, permissions, pinned digests and transactions hold even if a check misses something. Native plugins are the stated exception: they are protected by review and their full-access label (section 9) |
| One of everything | One validator, one store, one dispatcher, one manager component, one catalogue |
| Apps own their data | Documents, transactions, colour pipelines and file formats stay in each app |
| Proven practice over invention | Patterns used: OpenFX/CLAP, Flathub, TUF, SQLite, the transactional outbox, VS Code bisect, powerbox pickers, npm/pnpm minimum release age |
| Small shippable steps | Every delivery unit works on its own and is proven before the next |

## 3. Vocabulary and lifetimes

| Term | Meaning | Owner and lifetime |
|---|---|---|
| Extension | UI umbrella word for anything installable. "Add-on" is a synonym | — |
| Package | The install and update unit: one ID, one version, one archive | Store |
| Release | One version of a package: ID + version + pinned digest; immutable | Store, until cleanup |
| Contribution | One feature inside a package: `<package-id>/<contribution>` (e.g. `io.github.ink.grain/halftone`) | Package author |
| Plugin | A contribution with executable code (WebAssembly or native) | — |
| App setup | Per-app choices: contributions on or off, grants, settings, theme, optional pinned release | User, per app; survives updates |
| Saved feature | A plugin feature stored in a project: contribution ID, exact version, contract version, parameters, versioned state | The project |
| Runtime instance | A loaded module running under a principal | Memory only |

Grants attach to the **package within one app**. Enablement is per contribution and controls what appears; it is not a security boundary. The runtime instance enforces authority (T01).

## 4. What plugins can and cannot do

**Plugins can:** call every operation the app exposes to its CLI and automation, except the protected ones below. They can add effects, filters, generators, tools, panels, dialogs, commands, menu items, inspector sections, settings pages, canvas widgets, **new kinds of document objects** (section 16), importers and exporters, presets, themes, resources and workflows. They can edit documents within their granted scope, and they can be native.

**Plugins cannot:** replace, remove, disable, re-bind or alter built-in commands, tools, effects, menus or shortcuts. They cannot change default settings or app preferences, built-in file-format handlers, undo and history, updates, plugin management or permissions. They cannot reach the app's automation channels, and they cannot borrow another plugin's authority.

Operations carry one of four markings (section 7): normal (available to plugins), `Management`, `Core` (changes built-in behaviour) or `Internal`. Only normal operations reach plugins.

**Interaction styles** (T02):

| Style | Runs how | Can reach |
|---|---|---|
| Transform | Host passes a typed input and validates the typed output | Nothing beyond its contract |
| Session | Calls the app's exposed operations inside host-owned transactions | Exposed operations, within its grant and captured scope |
| Data | No code; read by a typed importer | Nothing |

Effective authority of a runtime instance = its style's imports ∩ the package's grant in that app ∩ what the host supports ∩ the invocation's scope ∩ machine policy.

## 5. System map

```text
storytold/craft-extensions  (all shared platform code, one version line, one CI)
  craft-ext-core          manifest, IDs, compatibility, the one validator
  craft-ext-store         installed releases, per-app setup, grants, leases, catalogue client
  craft-ext-ops           Invocation, principals, descriptors, authorize, errors, jobs, exit codes
  craft-ext-contracts     shared domain contracts + checked-in app contract inventories
  craft-ext-host          wasmi engine profile, loading, linker, budgets, native helper protocol
  craft-ext-conformance   contract suites, hostile-module suite, benchmarks, reference host
  craft-ext manager UI    the shared egui manager component
  craft-guest, craft-guest-<contract>   author SDK and generated bindings
  craft-ext (CLI)         create, check, test, dev, pack, prepare-release, verify

each app repository
  app adapter             contract export, domain contract implementations, operation markings,
                          UI extension points, project and job integration, embedded manager,
                          `plugin` CLI commands, headless test entry

catalogue repository (planned; maintainers to create and name)
  metadata only: publishers, packages, releases, revocations, transfers, policy

author repositories on GitHub
  source + GitHub Release assets (the only place plugin files live)
```

Crate names are illustrative. `craft-ext-core` and `craft-ext-store` already exist.

## 6. Contracts

**Three layers** (T02):

| Layer | Contents | Shared by |
|---|---|---|
| Common mechanics | Identity, parameters (units, ranges, animation), declarative UI, results and error codes, jobs, handles and revisions, edit proposals, logs | All apps |
| Shared domain contracts | `raster`, `temporal`, `audio`, typed resources (LUT and others); `path2d` is a candidate | Apps whose adapter passes the contract's conformance suite |
| App contracts | The app's own operations and extensions, generated from its Rust registry | That app only |

- **Shape.** Each contract is a small core plus negotiated optional extensions, as in OpenFX and CLAP. The plugin declares what it supports and needs. The app advertises versions, extensions and limits.
- **One module, many apps.** A transform written against a shared domain contract runs unchanged in every app whose adapter passes that contract's suite. Session contributions are always per app.
- **Host obligations:**
  - honest format conversion (no silent CMYK or Lab conversion);
  - supply or refuse regions, never seams;
  - evaluate keyframes and units;
  - validate output;
  - commit through the app's own undo;
  - isolate instances.
- **Plugin obligations:**
  - deterministic output;
  - tile invariance;
  - honest declarations.

  The SDK tests and conformance suites check these.
- **Versions.** A minor version adds optional items. A major version is a new contract that apps can implement alongside the old one, with a 12-month overlap (section 23). An unforeseen need starts as an app extension and is promoted when a second app needs it.
- **Pipeline (T03).**
  1. An app's `xtask export-contract` writes an inventory from its registry.
  2. App CI fails on breaking changes made without a major bump, as `cargo-semver-checks` and `buf breaking` do.
  3. The inventory is copied by PR into craft-extensions.
  4. The generator emits a committed guest crate whose major version equals the contract major.

## 7. One operation path

**Registry.** Each app extends its existing command registry; there is no second one. Each operation descriptor gains:
- `args` and `result` types with serde and schemars;
- `kind`: Query, Edit, Job or Effect;
- `exposure` by caller kind;
- the marking: normal, Management, Core or Internal;
- `requires(resolved)`, which computes needed permissions from the real targets and arguments;
- `interaction`: none, optional or required;
- version metadata.

Default exposure: an operation exposed to the CLI or automation is exposed to plugins unless it is Management, Core or Internal. CI checks that every operation is classified.

**Dispatcher.**

```text
fn execute(&mut self, inv: &Invocation, op: OpId, args: Args) -> Result<Outcome, OpError>

Invocation { principal, origin chain, captured scope, deadline, cancel, transaction }
principal = User(Gui) | Cli | Automation(client, scope) | Plugin(app, package, release, contribution, grant generation)
```

The old caller-less `execute(id, params)` is removed, so a new entry point cannot bypass the checks.

**Three checks in order:** exposure for the caller kind, then authorization with the resolved arguments, then availability. Availability is a UI hint and error message, never security. Explicit targets never skip authorization. Every denial happens before any side effect.

**Transactions.** An Edit opens a transaction and commits one undo step labelled with its origin ("Halftone · Ink & Grain"). It rolls back on error, trap, cancel or budget exhaustion, and nested calls join the outer transaction. Effects (file writes, network, processes) are queued and run only after commit (outbox). Plugins cannot reach undo or history operations.

**Long work.**
- A short command holds the document briefly.
- A Job reads a captured revision, works off the editing thread and commits with a revision check. If the document changed, the operation refuses or merges where it is provably safe.
- Events reach observers only after commit. A plugin reacting with an edit makes a new invocation, and origin chains are capped.

**Origin everywhere.** The principal is recorded in the journal, undo labels, logs and events. Plugin and agent actions do not train habit learning. A replay runs as the person replaying it. A batch is one invocation, authorized step by step.

**Handles, not paths.** Plugins get opaque, revision-carrying handles and artifact roles. User-picked files arrive as tokens, and stale handles fail.

**Data paths.** The control plane is JSON validated against the schema, with unknown fields rejected and size limits. Bulk pixels, audio and geometry use contract buffers and handles. Session plugins resolve their imported operations at load, so a missing operation is reported then, never mid-edit.

**Generated surfaces.** The same inventory produces SDK bindings, CLI `ops list` and `op <id>` with help, MCP tool schemas and reference docs. Existing CLI commands stay as aliases.

**Automation authority (T04).** A CLI the user runs directly is the user, with full authority including management. Clients attached to a running app (MCP, control channel, attached CLI) get the user's full document authority. Management for them needs an in-app approval, once per session.

## 8. Permissions and consent

**Vocabulary.** A permission = family + access + scope. The families are shared by every app:

| Family | Access | Scopes |
|---|---|---|
| Document | read, edit | selection, current document, all open documents |
| Files | read, write | picked item, picked folder (remembered), own storage |
| Network | connect | named https domains; never loopback, private ranges or the app's channels |
| Account | use | **not in the first release** (2026-11-10). When added, the host runs sign-in and holds tokens; the plugin never sees credentials |
| Clipboard | read, write | on a user action |

- **Always free, never asked:** computing on inputs, returning results, progress and logs, bounded private storage, UI placement.
- **Never grantable to WebAssembly plugins:** processes, native code, raw sockets, other plugins' data, management.
- **App-specific needs** add scopes to existing families. A new family needs a second consumer.
- **Network rules.** The host makes every request: https only, to declared domains only; redirects must stay within declared domains; response size and time are capped; loopback, private and link-local addresses are refused after DNS resolution, so DNS tricks cannot reach the local network or the app's own channels.
- **Private storage.** Each package gets a folder per app inside the store, with a size cap set by the host (raisable like time budgets). It is deleted on uninstall, kept on disable, and never readable by other packages.

**Required and optional.** Required permissions must be granted before a contribution can be enabled. Optional ones return "permission needed", and the SDK forces authors to handle that case. **Picking is permission:** a file or folder chosen in the host's dialog grants access to that item.

**When users are asked.**
- **At setup in an app:** one sheet with plain host-written sentences. Optional permissions are switches, off by default. Danger combinations are spelled out, for example "could send your document to api.example.com". An author's "why" appears quoted beneath the host's sentence.
- **Optional permissions:** at first use, only from a user-started GUI action, in a host-drawn prompt.
- **An update that widens permissions:** the new release waits and the old one keeps working until the user accepts.
- **CLI, agents and renders:** they never prompt. They fail with the exact grant to add.

**Grant record.**
- Each record holds app, package ID, source binding, normalized permissions, picked-folder tokens, a **generation**, the granted release and a date.
- Each host call checks the generation, so a revoke takes effect at the next call. Running edits roll back and jobs cancel. A queued job checks when it starts.
- Grants live on the machine, never in projects.

**Management.**
- In the GUI: rights per plugin with last use and a revoke button. Disable keeps grants; uninstall deletes them.
- The user's CLI: `plugin grant|revoke|permissions` as separate commands. There is never an `--allow-all` on execution commands.
- Agents need the management scope. An optional machine-wide policy (for example "no network for plugins") applies to everyone.

**Typical outcomes.** An effect asks for nothing. A panel that edits the selection shows one sheet. An uploader shows one sheet with the domain and the danger combination.

## 9. Runtime and safety

**Engine.** **wasmi 2** for all apps. EffectCraft, PhotoCraft and FigCraft already use it. `craft-ext-host` owns one profile that apps cannot loosen:
- fuel on;
- strict limits;
- a recursion cap;
- a pinned feature list (no threads);
- deterministic mode for transforms;
- validation caps;
- unknown imports refused.

Validated modules are cached per digest. The host is engine-neutral, so a compiling engine (wasmtime) can be added on desktop if benchmarks show it is needed (section 19).

**Budgets** start from PhotoCraft's pattern:

| Budget | Mechanism |
|---|---|
| Instructions | Fuel scaled by work size (pixel, sample, cell, node) |
| Time and cancel | Fuel in slices with deadline and cancel checks between them |
| Memory | Per instance and total across parallel instances; parallelism reduced to fit |
| Host calls | Fuel per call plus per byte copied |
| Output | Validated by the contract before use |

Profiles are preview, final render, session command and job, with numbers from benchmarks. "Allow more time" raises a plugin's budget per app, within the host's maximum.

**Instances.**
- **Transforms:** one per render thread, reused only within one document and job.
- **Session plugins:** on a worker thread, never the UI thread.
- **Trapped or over-budget instances** are discarded.

**Failure contract.**

| Event | User sees | Document |
|---|---|---|
| Trap | Message naming the plugin; preview shows an error badge and passes the frame through | Rolled back |
| Too slow or too much memory | "Took too long", with Allow more time | Rolled back |
| Invalid output | "Returned an invalid result" | Nothing committed |
| Forbidden call | Structured denial the plugin can handle | Unchanged |
| Revoked mid-run | Stops at next host call | Rolled back |
| Failure during final render or export | **The export fails**, naming plugin and frame | — |
| Repeated failures | Paused for the session with a notice | — |

The host never panics because of a plugin.

**Proof.** A hostile-module suite runs in CI on every desktop target and the web build. It covers infinite loops, memory bombs, deep recursion, huge tables, unknown imports, traps in every callback, invalid outputs, calls after revoke and cancel mid-work. Protection is claimed only where the suite passes.

**Native plugins.**
- Native plugins (helpers, OpenFX- or VST-style hosts) are allowed, in the store too. They run only out of process through a versioned helper protocol, never loaded into the app, so crashes are contained.
- They are labelled "Native: full access to your computer", with their own consent screen. Two maintainers approve every native listing and release; the bot never auto-merges them. Updates always ask, and a revocation kills the process.
- OS confinement (macOS App Sandbox, Windows AppContainer, Linux Landlock or bubblewrap) is an optional later improvement per platform. Without it, native plugins can do anything the user's account can, so the protected core holds for them only through review.

**Industry-format hosts kept (2026-11-10).** SoundCraft's VST3, CLAP and AU hosting and CADCraft's AutoLISP stay, because users expect these standard formats. They are treated as native full-access plugins: labelled, listed and managed in the shared manager. AutoLISP loses its ability to redefine built-in commands (`defun` over a built-in), so the protected core holds.

**GPU and audio.** Plugins call host GPU operations; plugin shaders come later. Craft's own real-time audio contract waits for SoundCraft evidence.

## 10. Identity and compatibility

**IDs.** Reverse-domain IDs, as the apps use today, with no UUID.
- `io.github.<owner>.<name>` is verified automatically against the GitHub repository owner. Other domains are verified once by a maintainer, for example with DNS, as Flathub does.
- First-party prefixes are reserved.
- IDs never change; display names and URLs can.
- A local file cannot take over a store-installed ID.

**Changes over time.**

| Event | Result |
|---|---|
| Repository renamed or moved | Numeric repository ID matches; URL updated |
| Plugin renamed | Display name only |
| New owner | Recorded transfer; users see "has a new owner" and confirm before the next update |
| Abandoned | "No updates since …"; a fork is a new ID |

**Compatibility** is decided by contracts, not app versions.
- Each package declares, per app, the contracts it uses (major and minimum minor), recorded automatically by `craft-ext pack`. A minimum app version is optional.
- Checks: manifest schema (critical changes bump it), contract versions, engine features, platform (native).
- Outcomes: **compatible**, **partly compatible** (each unavailable contribution with its reason) or **incompatible** (with the update to make). Never a silent half-load.

**Dependencies (2026-11-10).** A package may require **data packages** (resource packs, LUT packs, models, theme packs) by ID and version range. Installing offers the missing ones in the same flow, each with its normal labels and consent; removing a package that others need asks first. Version conflicts are reported, never resolved silently; cycles are refused. Shared code is compiled into each plugin (Rust crates), never shared at runtime. Plugin-to-plugin calls are a later, separately designed feature.

## 11. Store and lifecycle

The existing craft-extensions store already provides:
- a shared folder with a `CRAFT_EXT_DIR` override;
- immutable version folders;
- staging;
- a short lock;
- atomic state;
- a generation counter.

**Five kinds of state, one owner each:**

| State | Owner |
|---|---|
| Release bytes `packages/<id>/<version>-<digest>/` | Store; deleted only by cleanup |
| Package record (source, label, releases, default) | Store |
| Per-app setup (contributions, grants, settings, theme, pin, time raises) | User through any app, launcher or CLI |
| Leases (`leases/<process>.json`) | Each running app, CLI or job |
| Caches | Any app; always deletable |

**Transitions.**

| Action | Behaviour | After a crash |
|---|---|---|
| Install | Download → pinned digest → stage → validate → move → record; **enables nothing** | Previous state; leftovers cleaned |
| Update | **Always asks** (T08). New release beside the old one; per-app choices carry over; new contributions start off; widened permissions, new owner or native keep that app on the old release until accepted | Old release still default |
| Running work during update | Picked up at the next safe point. Effects switch on next use; panels reload when idle with a notice; **exports and jobs finish on the leased old release** | — |
| Versions kept | **Only the current release** plus any still leased. The user may choose **any listed version** to download, older ones included | — |
| Disable | Off in that app; grants kept | — |
| Uninstall | Record, setups and grants removed; bytes deleted when no lease remains; projects show placeholders | Record still present |
| Low disk | Free space checked before download | — |

**Labels and origins.** craft-extensions' existing tiers map to labels: `FirstParty` → Official, `ReviewedThirdParty` → store listing, `FromFile` → Not reviewed. A label is assigned by the host at install, never read from the package. Packages from `craft-ext dev` live in a separate development store; a normal app build that sees one treats it as Not reviewed.

**Schema evolution.** Two numbers, `write_version` and `read_version`, as SQLite's header has. Additive changes raise only the write number, so older apps keep reading; an app refuses to write a state newer than it understands. The first move from today's schema 1 keeps what current apps read in place.

## 12. Distribution and catalogue

**Store role.** The store **lists**. It does not build, host or take responsibility for plugins. Files come from authors' GitHub Releases.

**Hosting (2026-11-10).** The catalogue repository is on GitHub, so every listing is a reviewed PR with history. The signed catalogue is served as static files from **our own domain** (for example `plugins.getartcraft.com`) through a CDN, with GitHub Pages as a backup mirror. There is no server code and no database. Because the metadata is TUF-signed, the host cannot alter it, and the domain can move hosts at any time. Plugin files are not mirrored.

**Disclaimer and takedowns.** A short disclaimer in the catalogue README and the manager says listings are community plugins that the project does not build, host or guarantee. Takedown requests (copyright and others) are handled case by case under the listing rules (section 23).

```text
author: craft-ext prepare-release → GitHub Release on the author's repo (native: one zip per platform)
author: PR to the catalogue repo adding releases/<version>.toml
bot (no secrets, read-only): download, digest, validator, licence, source link, namespace, permission diff, smoke test
merge: bot for routine updates; quick human look for first listing, more permissions, new owner; two maintainers for any native release
publish job: catalogue.json regenerated and signed (TUF); auto-merged updates visible after 48 hours
apps, launcher, CLI: fetch catalogue (no login) → browse → choose version → download from the author → pinned digest → store
```

**Records** are metadata only:

| Record | Contents |
|---|---|
| Publisher | GitHub login and numeric owner ID, display name, official flag (maintainers only), status |
| Package | ID, repository and numeric repository ID, licence, apps, summary, native flag |
| Release | Version, tag, commit, per-platform URL/size/SHA-256, contracts per app, permissions, bot report |
| Revocation | Digest, package version range or publisher; advisory, block new or disable; reason |
| Transfer | Old and new owner; drives the new-owner notice |

All approved releases stay listed.

**Bot checks.**
- The shared validator, the pinned digest and size.
- The licence: MIT and/or Apache-2.0, with bundled code and assets from the house allowlist.
- A public source link at the tag, and namespace ownership.
- Diffs against the previous release.
- A WebAssembly smoke test in the reference host. Native releases get static checks, and a smoke test only on an ephemeral runner with no secrets.

**GitHub edge cases.**

| Event | Result |
|---|---|
| Release or repository deleted | Installed copies keep working; that version is "no longer available from the author"; nothing mirrored |
| Asset replaced | Digest mismatch: refused and flagged |
| Repository moved | Numeric ID matches; URL updated |
| Account re-registered by someone else | Numeric owner ID differs: listing frozen (blocks repo-jacking) |
| Offline | Cached catalogue; installed plugins unaffected |

**Local installs.** Any file can be installed, under any licence, with the same sandbox and permissions. It is labelled "Not reviewed". A digest that matches a listing is treated as that listing, and revocations apply by digest. Pilots use local installs until the catalogue exists (T18).

## 13. Authenticity and incidents

**Signing.** **TUF** through an existing Rust client.
- The trust root ships in the signed app.
- The online key lives only in the protected publish job, which needs a maintainer's approval.
- The offline root keys are held 2-of-3 by maintainers on hardware keys, with recovery rehearsed before launch.
- Expired metadata is shown as stale and cannot authorize installs. Verification failure never replaces trusted metadata. If the computer's clock appears to have gone backwards (earlier than metadata already seen), new installs and updates wait until it is corrected; installed plugins keep working.

**Threats and defences.**

| Threat | Defence |
|---|---|
| Catalogue changed by someone with repository or hosting access | Only signed metadata accepted; trust root in the app |
| Author's account taken over, routine-looking update | **48-hour hold** for auto-merged updates, as npm and pnpm do with a minimum release age; the PR notifies the author; anyone can report |
| Asset swapped | Pinned digest |
| Replay of old metadata | Version and expiry |
| Online key leaked | Root keys rotate it without an app update |

**Revocation.**

| Severity | Effect | Who |
|---|---|---|
| Advisory | Warning | Any maintainer |
| Block new | No new installs, enables or updates | Any maintainer, immediately |
| Disable | Stops loading; running work stops at next host call; native process killed; export in progress fails | Any maintainer immediately; a second reviews within a day |

Apps check at start and every few hours when online. Known revocations survive repair and restore. Project data is never deleted.

**Account-takeover steps.**
1. A report arrives.
2. The publisher gets block new, and disable if malicious code shipped.
3. Banners appear on the listing and in apps.
4. The recovery release gets a quick human look.
5. A public note is published.

## 14. Management in every app and the launcher

- **One shared egui manager component** in craft-extensions, embedded in every app (Settings ▸ Extensions and a menu item) and in the launcher. In an app it shows that app; the launcher shows all apps. **The launcher is never required.**
- **CLI parity:** `<app>-cli plugin search|info|versions|install|setup|grant|revoke|update|remove`, using the same client.
- **One shared store folder**, so installs show up in the app and the launcher alike.

**Screens.**

| Screen | Content |
|---|---|
| Browse | Search, filters (kind, works in this app, native), rows with name, summary, author, stars with date, labels (Official, Native: full access, Not reviewed), compatibility |
| Detail | Description, screenshots, permissions in plain sentences, per-app compatibility, versions to choose, licence, source, release notes |
| Install and setup | Cancellable download, then "Set up in <app>": contributions and permissions on one sheet; large resources offered (section 17); other apps show "New plugin ready to set up" |
| Installed | Per app: contributions, permissions with last use and revoke, version, update, remove, performance (section 19) |
| Updates | Count badge; notes with more permissions, new owner and native highlighted; always asks |
| Problems | Failed, paused, incompatible or blocked plugins with reason and fix button; safe mode; "Find the problem plugin"; Report plugin |

**Moving to a new computer.** The manager and CLI export and import a **plugin setup** file: packages, versions, and per-app on/off choices and settings. Import installs everything, but **permissions are asked again** on the new machine, so a copied file never grants access silently.

**Ranking.** Filter (query, app, kind, compatible, not blocked) → relevance tier (exact, prefix, text) → **official first within a tier** → stars (fetched centrally) → newer approval → ID.

**Built-in search suggestions.** When a built-in search finds nothing, matching plugins from the cached catalogue appear ("No built-in match. 3 plugins in the store."). The query never leaves the machine, and a setting turns this off.

**Builds.**

| Build | Store |
|---|---|
| Standalone download, launcher-installed | Shared folder |
| Linux Flatpak | Access granted to that one folder in the manifest; otherwise the app's own store, still fully managed |
| Web | Browser-local store per app; file installs; catalogue installs if GitHub downloads work cross-origin (unverified) |
| Builds without network code | Local file installs only, labelled |

Accessibility: everything is reachable by keyboard, states are exposed to screen readers, and labels never rely on colour alone. Text goes through `craft-i18n`. No account is needed for anything public.

## 15. Plugin UI

**The host draws everything and plugins describe it.** That gives matching themes, accessibility and localization for free. It also means no spoofed prompts and no plugin code on the UI thread. Webviews are not used in native apps (house rule).

| Kind | Author writes | Host does |
|---|---|---|
| Effect parameters | `#[derive(craft::Params)]` | Controls, keyframes, presets |
| Commands | A command contribution | Menus, command search, CLI, MCP |
| Menu items | A declared extension point (e.g. Effect ▸ Stylize) | Appended after built-ins, grouped by plugin |
| Shortcuts | Optional suggestion | Used only if the chord is free; never replaces built-in or user bindings (`craft-keymap`) |
| Panels, dialogs, inspector sections, settings pages | Declarative tree plus a **custom canvas** element (draw lists or an image) | Docked with `craft-ui`; plugin dialogs carry the plugin's name and icon |
| Canvas overlays and widgets | Bounded draw lists in document coordinates | Rendering, hit testing, hover and click events |
| Pointer tools | begin, update, commit, cancel | Capture, coalesced updates, overlay preview; commit is one undo step; Escape cancels unchanged |
| New document object types | A type with typed data, drawing or rendering, and inspector UI | Selection, transform, undo, save and collaboration; a fallback preview saved in the file (section 16) |
| Importers and exporters | A format handler | Built-in handlers stay the default for their formats; a plugin handler for the same format is offered as a choice and becomes default only when the user picks it |

**Behaviour.**
- Each event is an invocation under the command budget.
- Closing a panel disposes its session. A document switch sends a scope change and cancels a gesture.
- Update or disable reopens the UI at the next safe point.
- Panel placement is saved by the host, settings in the plugin's private storage, and widget data in the document.
- Headless, UI contributions do not exist but their commands remain.
- The SDK refuses unlabelled controls.

## 16. Projects and reliable output

**What a project stores.** Per feature: contribution ID, exact version, contract version, parameters and versioned plugin state. Per project: a **plugin list** of IDs, versions, and catalogue links or "local only". Grants are never stored.

**Reopening.**

| Situation | Result |
|---|---|
| Same or newer compatible release | Used, with a quiet note if the version differs |
| Old state schema | Plugin `migrate()`; saved only when the user saves; failure → placeholder |
| Older release or incompatible major | Placeholder with Update |
| Not installed or not set up | Placeholder; one prompt lists all ("This project uses 3 plugins") |
| Revoked, or from a removed old plugin system | Placeholder with the reason |

**A placeholder keeps everything:** parameters, state, keyframes, position. Preview passes the input through with a badge, everything else stays editable, and saving writes the data back byte for byte.

**Output.**
- A final render or export with any missing, failed or blocked plugin **does not start**. It lists each plugin with a fix (the CLI prints the install commands).
- Preview may use draft quality, but final always runs at full quality. Caches key on release digest, contract version, parameters, inputs and quality.

**Plugin object types.** A plugin can define a new kind of document object (a layer type in EffectCraft, a node type in FigCraft, an object type in VectorCraft). The app owns everything generic: selection, moving, undo, saving and collaboration. The plugin supplies drawing or rendering and its inspector. On save the app also stores a **fallback preview** (pixels, frames or geometry), as Illustrator does for plugin art. A project opened without the plugin shows that preview as a locked placeholder that keeps all the object's data; final export still refuses until the plugin is available or the object is baked.

**Plugins from the removed systems.** Old plugin systems are removed with release notes only (no compatibility layer, no porting by the project). Projects that used them open with placeholders that keep their data.

**Portability.**
- **Bake plugin result** converts output to the app's own data and keeps the original switched off, so it can be un-baked.
- **Include plugins** is an explicit, off-by-default option for saving or collecting files. Carried plugins are never installed or run on open; "Install from project" is an ordinary local install.
- **Collaboration** works because plugin edits are ordinary operations, and plugin state is plain data.

## 17. Resources, themes and workflows

Data packages contain no code. Each kind has one bounded importer, and installation runs nothing.

| Kind | Notes |
|---|---|
| Themes | Shared palette with per-app overrides (exists; WCAG contrast enforced); colours, fonts and density only for now; never auto-activated; **themes will be revisited later** |
| Presets | Contribution ID + parameters |
| LUTs, ICC profiles, gradients, swatches, patterns | Shared importers |
| Brushes, shapes, symbols, templates | App importers; templates through the normal loader |
| Fonts | Licences from the house allowlist; usable inside Craft apps only |
| Shortcut profiles | `craft-keymap` profiles; applied by choice; cannot bind hidden or management commands |
| Translations | `craft-i18n`; never replace permission, consent or warning text |
| ML models | `craft-ai-core` store |
| Workflows | Graphs of operations and contributions; inspectable without running; run as jobs **with their package's declared permissions**; missing nodes become placeholders |

**Scripts and expressions.** The apps' own scripting and expressions (for example EffectCraft's Application Scripting) stay app features: they run as the user (section 7) under the app's script permission, and they are not store packages. Store packages are compiled plugins. A sandboxed scripting language for plugins, running inside WebAssembly with the same permissions, is a later contribution kind.

**Large files** are referenced by URL, size and SHA-256 (GitHub caps assets at 2 GiB).
- Setup offers **Download now** (primary) or **When first needed**. First use shows the size, and the CLI takes `--with-resources`.
- Downloads resume and are digest-checked. Storage is content-addressed and shared between packages, and a file is removed when nothing refers to it.

## 18. Automation and jobs

**Environments.** Fresh CLI, attached (controls the running app) and server (headless, long-running) share operations, results and error codes. A fresh CLI uses the plugins set up for that app and never prompts. It refuses to write a project open in a running app and suggests `--attach`.

**Attaching.**
- Attaching goes through a Unix domain socket in a user-only directory, or a named pipe restricted to the user, plus a per-session token in a user-only file. There is no network port by default; a remote server mode can come later, with TLS and a token.
- While any external client is connected, the app shows "**<client> is controlling <app> · Stop**".
- Several clients may attach. Plugins have no sockets and never receive tokens.
- This replaces today's unauthenticated localhost TCP ports.

**Jobs.** One model everywhere: ID, owner, state (queued, running, succeeded, failed, cancelled), progress, outputs and error.

| Client | Progress | Cancel |
|---|---|---|
| CLI | Human output or `--json` lines | Ctrl-C |
| MCP | `notifications/progress` | `notifications/cancelled` |
| Attached | `jobs list`, `jobs wait` | `jobs cancel` |

Attached jobs continue if the client disconnects and can be rejoined by ID. On quit, the GUI asks; headless cancels cleanly.

**Outputs and errors.** Output is written to a temporary name and renamed when complete. Partial output is deleted on failure or cancel. Exit codes are shared with MCP errors:

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | Failed |
| 2 | Usage |
| 3 | Input required, listed |
| 4 | Plugin missing or not permitted, with the command to fix |
| 5 | Cancelled |

## 19. Performance, offline and diagnostics

**Budgets.**
- Budgets come from a CI benchmark set (blur, halftone, LUT apply, panel callback, geometry edit) on reference hardware: measured cost × generous headroom per profile.
- There is an app-wide plugin memory ceiling. Interactive and preview work outranks background export.
- **Engine re-evaluation trigger:** a reference 4K effect several times slower than its built-in equivalent, dropping preview below real time.

**Speed.** Validated-module cache, per-job instance reuse, parallel tiles, counted and minimized buffer copies, separate preview and final caches, and deterministic output that can be cached across machines.

**Visibility.** Per-plugin cost and memory appear on the Installed screen, with an optional viewer overlay. Authors see the same numbers in `craft-ext dev`.

**Offline.**

| Works | Needs a connection |
|---|---|
| Installed plugins, projects, placeholders, bake | Install, update, version change |
| Cached catalogue (dated) | Fresh revocations (cached ones apply) |
| Plugins' private storage | Large files not yet downloaded |
| No sign-in ever for public plugins | Network-permission plugins get an "offline" error |

**Logs and support.**
- Each app keeps a local, size-capped, rotated log. Plugin lines are tagged and treated as untrusted, and paths, URLs and content are redacted by default.
- Authors see their plugin's output in a **Plugin console** panel.
- **Create support report** is an explicit, user-reviewed action. There is no automatic upload and no crash telemetry.

**Safe start.**
- If the last start did not finish loading plugins, the app offers "Start with plugins off?".
- Manual entry points: `--safe-mode` and Help ▸ Restart with plugins off.
- In safe mode no plugin code or plugin themes run, the manager still works, and projects keep plugin data.
- **Find the problem plugin** bisects across restarts.

## 20. SDK and author experience

- **Declarations.** `Craft.toml` holds what a reviewer reads as a whole: ID, name, version, licence, publisher, links and **permissions per app**. Contributions and parameters are declared in code with `#[craft::transform(...)]` and `#[derive(craft::Params)]`. The macros write a WebAssembly custom section, so the manifest and code cannot drift. `check` computes required permissions from actual imports and prints the exact line to add. Permissions are never inserted automatically.
- **Toolchain.** Rust first, `wasm32-unknown-unknown`, pinned by `rust-toolchain.toml`. Target features match the wasmi profile (still to verify). Builds are deterministic, with `cargo-deny` and an SBOM. Other languages come later, only with generated and verified bindings.
- **Testing at three levels.**
  - Unit: native `cargo test`.
  - Contract: `craft-ext test` in the reference host, with determinism, tile invariance, formats, ranges and cancellation.
  - App: `<app>-cli plugin test` against fixtures.
- **Development loop.** `craft-ext dev --app <app>` builds, packs and installs into an isolated development store. It starts or attaches to the app, streams logs and reloads on change.
- **Release.**
  - `craft-ext prepare-release` builds, checks, tests, writes the zip, inventory and catalogue record, and reports differences from the previous release (removed IDs, state changes without migration, wider permissions).
  - A template GitHub Action uploads the release and adds attestations.
  - The author opens the catalogue PR. The tool never publishes on its own.
- **Distribution of the SDK.** crates.io is intended, but publishing needs authorization; git tags serve until then.
- **Trust.** The SDK is a convenience, never a trust anchor. The bot, `check` and the installer share one validator.

## 21. App adapters

**There is one adapter per app (not per plugin)**, in the app's repository. It translates between the shared platform, which knows plugins, permissions, the sandbox, the store and contracts, and the app, which knows its documents, undo, rendering and menus. It contains no copy of shared logic, so a plugin behaves the same way in every app.

**What it does.**

| Job | Section |
|---|---|
| Says what the app offers: contract export, the shared domain contracts it implements, its CI check | 6 |
| Converts data honestly between the app's types and contract types, and validates results | 6 |
| Marks every operation: kind, exposure, `requires`, normal/Management/Core/Internal | 7 |
| Places contributions in the app's registries, menus, panels, inspector, tools and canvas | 15 |
| Runs them through the app's undo, render pipeline, caches and job queue | 7, 16, 18 |
| Saves and reopens: references, placeholders, fallback previews, export refusal, bake | 16 |
| Embeds the shared manager, the `plugin` CLI commands and the socket-and-token attach transport | 14, 18 |
| Proves it works: conformance runs and a headless plugin-test entry | 20 |

**How it works, for one effect.**
1. At start the adapter asks the store which plugins are installed, set up for this app, compatible and not blocked, and registers their contributions.
2. Adding the effect is a normal operation with one undo step.
3. At render the pipeline asks the adapter. The adapter converts the frame to the contract, runs the plugin through `craft-ext-host` under budget, checks and converts the result, and returns it for caching.
4. On save the adapter writes the reference.
5. On update it unloads at a safe point. Running exports keep their leased release.
6. If the plugin is missing on reopen, it shows a placeholder and refuses export.

**Levels (2026-11-10).** An app's plugin support grows in levels, and the manager shows each app's level.

| Level | Adds | Rollout |
|---|---|---|
| L0 | Manager, store, `plugin` CLI, data add-ons (themes, presets, LUTs, fonts and the app's resource kinds) | **Every app, early** |
| L1 | Transforms through shared domain contracts (effects, filters, generators, functions) | Per app, by demand |
| L2 | Session plugins: exposed operations, panels, tools, canvas, new object types | Per app, by demand |

**Shape (2026-11-10).** `craft-ext-host` defines small traits, one per job: contract provider, operation markings, UI extension points, project hooks and job hooks. Each app has **one adapter crate** (for example `effectcraft-plugins`) that implements them. A generated starter skeleton and a checklist test fail when a required hook for the app's level is missing.

**Marking is mandatory (2026-11-10).** The contract export check fails CI for any operation without a plugin marking, so nothing is exposed or hidden by accident.

**Who builds them (2026-11-10).** The platform team builds the EffectCraft, FigCraft and PhotoCraft adapters together with those apps' maintainers, and turns what it learns into the skeleton and an adapter guide. Every other app's maintainers then build and own their adapter.

**Per-app focus.**

| App | Offers | Hard part |
|---|---|---|
| EffectCraft | raster, temporal, own operations | Time, keyframes, seeking, unloading effects without restart |
| PhotoCraft | raster | Colour modes, bands, smart filters |
| FigCraft | own operations (session) | Panels, tools, canvas widgets, object types |
| VectorCraft | object and path contract | Units, geometry validation |
| FilmCraft | temporal, timeline operations | Timebase, export jobs |
| SoundCraft | audio (later); VST3/CLAP/AU kept as native | Real-time rules |
| CADCraft | own operations; AutoLISP kept as native | Step-by-step input prompts as headless parameters |
| GridCraft | functions, workbook operations | Recalculation, cycles |
| Others | own operations and data kinds | Per each app's own work package |

The app's old plugin system is removed in the same change set that lands the adapter. Registries that cannot unload a contribution until restart (EffectCraft's effect slots today) must gain unload, so updates can apply at safe points.

## 22. Delivery plan

| # | Unit | Where | Proof before moving on |
|---|---|---|---|
| P0 | Accept, name owners, plan hold lifting | Maintainers | Owners named |
| P1 | Store foundation (section 11) and manifest schema 2 | craft-ext-core, craft-ext-store | Crash at each step, concurrent writers, old reader, update during leased job |
| P2 | Operations kit (section 7) | craft-extensions | Denial before side effect; management unreachable by plugins |
| P3 | Host runtime and hostile suite (section 9) | craft-ext-host, craft-ext-conformance | Suite passes on all desktop targets and web |
| P4 | First contract `raster.filter@1` and SDK; example halftone | craft-extensions | Two builds give the same digest |
| P5 | Manager component and `plugin` CLI (local installs) | craft-extensions | Full lifecycle driven headless |
| P5b | **L0 in every app**: adapter crate skeleton, manager, `plugin` CLI, data add-ons | Each app | Manager and a theme or preset install work in each app |
| P6 | **EffectCraft pilot**: `Invocation` migration, socket attach, raster and basic temporal adapter, manager, placeholders, export refusal; old plugin crate, Plug-in Library and helpers removed | EffectCraft | GUI/CLI/MCP parity; export refuses missing plugin; no open TCP port |
| P7 | Catalogue: repository, bot, TUF, 48-hour hold, revocation, client fetch (download routine extracted from the launcher) | New repository, craft-ext-store | Swapped asset and expired metadata refused; revocation applied offline after sync |
| P8 | **FigCraft**: session style, panels, tools, canvas, consent UI; APIs 1–4 removed | FigCraft | Panel/CLI parity; Escape cancels; one undo step per edit |
| P9 | **PhotoCraft**: the same halftone module unchanged; old filter ABI removed | PhotoCraft | Same output as EffectCraft |
| P10 | Native helper protocol and full-access label | craft-ext-host, EffectCraft | Native crash contained; revocation kills process |
| P11 | Other apps by demand (FilmCraft next); themes revisit | Each app | Per-app proofs |
| P12 | Public launch: catalogue domain live, keys held, listing rules and disclaimer, author guide, templates, independent outside security review of the platform | All | Maintainers decide community enablement |

Until P2 lands, pilots ship transform plugins only. The `third-party` cargo feature stays off until P7. P1, P2 and P3 can run in parallel. P5b and P7 run alongside P6–P9. A maintainer merge hold on plugin-system changes lifts area by area as units land; EffectCraft's plugin areas lift at P6.

## 23. Policy and maintenance

| Area | Policy |
|---|---|
| Licences | Store: MIT and/or Apache-2.0, with bundled code and assets from the house allowlist. Local installs: any, labelled |
| Listing rules | Refused or removed only for malicious or deceptive behaviour, impersonation, illegal content, failed bot checks or unverified namespace. Never for quality, competing with built-ins or being native |
| Appeals | In the catalogue repository; a different maintainer decides, normally within 14 days; emergency blocks stand meanwhile |
| Official plugins | Same catalogue, rules and licences; flag set by maintainers only |
| Paid or private distribution | Not a store goal; no credential or entitlement integration |
| Disclaimer and takedowns | Short disclaimer in the catalogue README and manager; takedowns case by case under the listing rules |
| Security review | Independent outside security review of the platform (sandbox host, store client, catalogue signing, attach transport) before public launch (P12) |
| Native review | Two maintainers approve every native listing and every native release; the bot never auto-merges native |
| Contract support | A contract major stays supported for **12 months** after its successor ships. It can end earlier only for published security reasons. Authors get warnings from `check`; users are warned only in the final months |
| Contract changes | PR in craft-extensions; shared domain contracts need approval from maintainers of at least two consuming apps |
| Ownership | Platform maintainers (craft-extensions); each app's maintainers (adapter and contract); catalogue maintainers (repository, bot, keys, incidents). Names to be assigned |
| Docs | Generated contract reference and changelog (docs.rs once published, GitHub Pages until then); hand-written author guide, templates per contribution kind, examples, listing rules, incident process |
| Cost | No servers: GitHub repositories, Actions and Pages. Ongoing: maintainer time and CI minutes |

## 24. Open items

| Item | Owner | When |
|---|---|---|
| Name owners for every part; create and name the catalogue repository; assign key holders | Maintainers | P0 / P7 |
| wasmi 2 accepted WebAssembly features for the SDK template | Platform | P3–P4 |
| A maintained TUF Rust client and its licence | Platform | P7 |
| Budget numbers from benchmarks | Platform | P3, P12 |
| egui accessibility coverage for the manager and declarative UI | Platform | P5, P8 |
| GitHub release downloads from browsers (web builds) | Platform | P5 |
| Themes beyond colours, fonts and density | User | Later, by request |
| Account sign-in broker (OAuth in the system browser, tokens in the OS keychain) | Platform | When the first account-based plugin is wanted |
| Plugin-to-plugin calls and sandboxed script plugins | Platform | Later contribution kinds |
| Future backend encoding (for example a component model) | Platform | Only if a later backend needs it |
| crates.io publication of the SDK | Maintainers | At first SDK release |

## 25. What this replaces

| Earlier position | Now |
|---|---|
| Preserve existing app ABIs as legacy profiles; PhotoCraft first-party slice first | Existing app plugin systems are removed and replaced; pilots run EffectCraft → FigCraft → PhotoCraft |
| Package UUIDs and legacy ID aliases | Verified reverse-domain IDs; no legacy mapping |
| Plugin exposure opt-in per operation | Every CLI/automation operation, except Management, Core and Internal |
| Proprietary and restricted distribution in the store | Open-source store (MIT/Apache); any licence locally |
| Human review of every release; store rebuilds | Light checks; bot merges routine updates with a 48-hour hold |
| Native backends unavailable until confinement is proven | Native allowed with a full-access label; confinement optional later |
| Install enables every listed app; update deletes the old release | Install enables nothing; leases keep releases in use |
| Unsigned indexes; unauthenticated localhost control ports | TUF-signed catalogue; user-only sockets or pipes with tokens |
| Missing effects dropped at render | Placeholders keep data; export refuses |
| A full private plugin server considered | GitHub repository plus static signed files on our domain |


## Where the detail lives

- Every decision with its date and wording: [DESIGN-DECISIONS.md](DESIGN-DECISIONS.md)
- Each topic's proposal, with evidence and rejected alternatives: [TOPIC-PROPOSALS.md](TOPIC-PROPOSALS.md)
- Comparison with this repository's code and threat model: [ARCHITECTURE-GAP-ANALYSIS.md](ARCHITECTURE-GAP-ANALYSIS.md)
- Current security gates: [THREAT-MODEL.md](THREAT-MODEL.md)
