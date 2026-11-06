# ArtCraft Monorepo

ArtCraft monorepo containing all 19 projects, including the integrated FilmCraft upstream, Mac Codex, and PC changes.

Unified **ArtCraft & Craft Suite** repository containing all 19 applications, libraries, services, and reverse-engineering tools under their respective project directories:

## Changes from the original ArtCraft

### Web changes on GitHub

| Change | What changed | Source |
| :--- | :--- | :--- |
| Offline model preview | The Image and Video screens each list 22 models from the repository. Each screen keeps its selection while you switch between them. The preview works without an API and disables Generate. | [Build and run the preview](https://github.com/hyegonKim/ArtCraft/tree/2b705ef942760d3b37b007cac55fa0e807958cbd/artcraft-services/frontend/apps/artcraft-webapp/model-preview) |
| Login form validation | Empty credentials and whitespace-only usernames are rejected before an API request. The form announces the validation message to assistive technology. | [Login fix and regression tests](https://github.com/hyegonKim/ArtCraft/commit/a0208f9b73af081b2383ac717200dde84ffe8b72) |

### Changes verified in local builds

The following summarizes the local rebuild and its verification scope. The GitHub links above identify the web changes available on their feature branches.

- **Original-resolution editing and PNG export.** ArtCraft Native replaced the fixed 320 × 200 canvas with a document sized to the imported image. Editing and PNG export preserve the document dimensions and transparency. Fit, 100%, zoom, and pan control the view.
- **Layer editing and project files.** Added linked layer masks, non-destructive transforms, RGB and per-channel Curves, continuous brush and eraser strokes, and gesture-level Undo/Redo. Editable `.acraft` files preserve layers, masks, transforms, Levels, and Curves.
- **Video frame extraction.** The web tool can capture the first frame of an imported MP4 before playback. Browser verification covered stepping one frame forward, capturing again, and saving a 640 × 360 PNG.
- **Desktop workflows.** Verified the basic launch and interaction flows for ArtCraft Native and 13 Craft Suite tools. ArtCraftX verification also exercised five workspaces and four native Tauri read commands.
- **Ghidra integration.** Built the Rust engine and verified it against the actual Ghidra SDK. The integration covered 16 analysis cases and seven production CLI checks.

The native editor verification covered editing, PNG export, Undo/Redo, and reopening a 4097 × 2305 project on macOS. The web preview was tested in a browser with API access blocked. Live account sign-in, billing, and paid AI generation require a configured backend. The Adobe products listed below are workflow references.

## Applications & Suites

| Directory | Project | Adobe / Industry Counterpart |
| :--- | :--- | :--- |
| [`artcraft/`](./artcraft) | **ArtCraft (`artcraft-native` & Desktop)** | Adobe Creative Cloud / Firefly AI Studio (Photoshop v27.11 GPT Image 2 & Edge Enhancement) |
| [`photocraft/`](./photocraft) | **PhotoCraft** | Adobe Photoshop v27.11 / LTS v26.11.8 & Camera Raw v18.7 |
| [`vectorcraft/`](./vectorcraft) | **VectorCraft** | Adobe Illustrator v30.8.2 / LTS v29.8.11 |
| [`filmcraft/`](./filmcraft) | **FilmCraft** | Adobe Premiere Pro v26.5.2 / v25.6.6 / v24.6.4 & Media Encoder v26.5.2 (`hyegonKim/filmcraft` Korean Transcript base) |
| [`lightcraft/`](./lightcraft) | **LightCraft** | Adobe Lightroom Classic & Camera Raw v18.7 |
| [`effectcraft/`](./effectcraft) | **EffectCraft** | Adobe After Effects 2026 |
| [`designcraft/`](./designcraft) | **DesignCraft** | Adobe InDesign 2026 |
| [`soundcraft/`](./soundcraft) | **SoundCraft** | Adobe Audition 2026 |
| [`figcraft/`](./figcraft) | **FigCraft** | Figma / Adobe XD |
| [`wordcraft/`](./wordcraft) | **WordCraft** | Adobe Acrobat DC / Pro & Word Processor |
| [`cadcraft/`](./cadcraft) | **CadCraft** | AutoCAD 2D/Parametric DCM |
| [`deckcraft/`](./deckcraft) | **DeckCraft** | Presentation & PDF Handouts |
| [`gridcraft/`](./gridcraft) | **GridCraft** | Spreadsheet & Formula Engine |
| [`craft-launcher/`](./craft-launcher) | **Craft Launcher** | Adobe Creative Cloud Desktop (Recent Updates & App Management) |

## Shared Libraries, Services & Tooling

| Directory | Description |
| :--- | :--- |
| [`craft-libs/`](./craft-libs) | Shared UI widgets, print, update, and platform crates |
| [`craft-i18n/`](./craft-i18n) | Localization & internationalization infrastructure |
| [`artcraft-services/`](./artcraft-services) | Backend services and asset pipelines |
| [`artcraftx/`](./artcraftx) | Extended ArtCraft experimental & cross-platform modules |
| [`ghidra-hopper-rs/`](./ghidra-hopper-rs) | Rust-native SLEIGH/P-Code decompiler & Hopper ByteType reverse-engineering engine |
