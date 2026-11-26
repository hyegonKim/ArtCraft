# Gaps

> **Last reviewed:** 2026-11-14 · **Last updated:** 2026-11-14 · **Change:** major (Auto Lip-Sync viseme engine + command + UI, XFL/`.fla` export round-trip, and built-in Korean `ko` UI localization added) · **Target:** the professional 2D vector & cutout animation workflow

## Current Capability Summary

- **Vector Stage, Brushes & Drawing Model (`anicraft-geom`, `anicraft-doc`, `anicraft-render`, `anicraft-gpu`, `src/`)**:
  - Merge Drawing and Object Drawing modes, planar edge-map subdivision for Paint Bucket / Ink Bottle / Faucet, variable-width strokes (`Width` tool + width profiles), pressure/tilt curves, Brush Library presets (`anicraft_engine::brush_library`), Pen/Subselection cubic Bezier editing, shape booleans/offset/simplify/smooth, and CPU + wgpu hardware-accelerated tiled stage rendering.
- **Timeline, Tweens, Onion Skinning, Symbols & Camera (`anicraft-doc`, `anicraft-engine`, `src/timeline.rs`, `src/panels.rs`)**:
  - Multi-scene timelines with Normal, Guide, Motion Guide, Mask, Folder, and **Camera** (`카메라`, pan/zoom/rotate + tint/colour effect + `Preview Through Camera`) layers.
  - Classic Tweens (with orient-to-path and custom cubic-bezier easing), Shape Tweens (with lettered Shape Hints `a`–`z`), and Motion Tweens (with per-property curve editing in the Motion Editor).
  - **Onion Skinning (`어니언 스킨`)** with past/future range handles, filled or outline mode, anchor markers, Symbol Onion Skin, and **Edit Multiple Frames**.
  - Graphic (`Loop`, `Play once`, `Single frame` + `First frame` picker), Movie Clip (independent looping timeline + blend modes + 7 bitmap filters: Blur, Drop Shadow, Glow, Bevel, Gradient Glow, Gradient Bevel, Adjust Colour), and Button (`Up`/`Over`/`Down`/`Hit` states + event actions) symbols with 9-slice scaling grids.
- **Rigging: Bone / IK Armatures & Asset Warp (`anicraft-doc`, `crates/engine/src/cmds_bones.rs`, `crates/engine/src/cmds_warp.rs`, `src/bone_tool.rs`)**:
  - **Bone & Bind tools (`뼈대` / `바인드`)**: symbol and shape armatures, FABRIK/CCD inverse kinematics with joint angle constraints, pinned joints, per-vertex skin weights (`Shape Weights`), and spring-bone secondary physics (`Spring Bones`).
  - **Asset Warp (`에셋 워프`)**: Delaunay-triangulated deformation cages with pin handles (fixed, rotation, depth) and keyframeable warp poses on shapes and bitmaps.
- **Audio & Auto Lip-Sync (`anicraft-audio`, `crates/doc/src/lipsync.rs`, `crates/engine/src/cmds_sound.rs`, `src/envelope.rs`)**:
  - WAV, Ogg Vorbis, and MP3 import, Event/Start/Stop/Stream sync modes, timeline waveform rendering, custom stereo volume envelopes, and stereo WAV mixdown (`file.export_audio`).
  - **Auto Lip-Sync (`자동 립싱크`, `sound.auto_lip_sync`)**: deterministic acoustic viseme analysis (RMS energy, zero-crossing rate, 3-band spectral energy shares, onset slope, and 1-frame jitter smoothing) mapping speech audio to the 12-viseme chart (`neutral`, `ah`, `d`, `ee`, `f`, `l`, `m`, `oh`, `r`, `s`, `uh`, `woo`) and built-in labelled mouth charts (`templates::MOUTHS`), stamping `SingleFrame` Graphic symbol keyframes with full undo support and Properties / `Modify › Timeline` UI triggers.
- **Interchange, Publishing & Localization (`anicraft-xfl`, `anicraft-svg`, `anicraft-engine`, `src/i18n/`)**:
  - **XFL / `.fla` Import & Export**: clean-room XFL folder and `.fla` ZIP importer (`import_path`, `import_fla`) plus clean-room XFL package & `.fla` ZIP exporter (`export_package`, `export_fla`, `file.export_xfl`) preserving scenes, layers, library symbols, cubic/quadratic edges, solid/linear/radial fills, tweens, bitmaps, WAV sounds, and ActionScript metadata.
  - **Multi-format Export & Publish**: Sprite Sheet (`PNG` + JSON frame atlas, `file.export_sprite_sheet`), SVG (`file.export_svg`), Animated GIF (`file.export_gif`), Animated PNG (`file.export_apng`), Animated WebP (`file.export_webp`), PNG Sequence (`file.export_png_sequence`), self-contained HTML5 Canvas (`file.export_html`), Lottie JSON (`file.export_lottie`), WebM/MP4 Video via `ffmpeg` (`file.export_video`), and WebAssembly exporter/tool plug-ins (`anicraft-plugins`).
  - **Localization (`src/i18n/`)**: `tr!` / `trn!` / `trc!` / `tr_noop!` across 100 % of shell UI strings, with built-in English (`en`), **Korean (`ko`, `한국어`)** catalog (`src/i18n/ko.tsv`) + desktop CJK/Hangul system font fallback (`Malgun Gothic`, `Apple SD Gothic Neo`, `Noto Sans CJK`, `NanumGothic`), and layout-testing pseudo-locale (`pseudo`).

## Remaining Gaps

Every known shortfall against the full desktop 2D animation authoring workflow, most important first.

| # | Gap | Evidence | Impact | Estimate |
|---:|---|---|---|---|
| 1 | Full ActionScript 3.0 / JavaScript VM execution in Test Movie | `anicraft-actions` parses and executes timeline navigation (`play`, `stop`, `gotoAndPlay`, `gotoAndStop`, `nextFrame`, `prevFrame`), variables, and button/frame handlers; arbitrary AS3 class hierarchies (`linkage`) are preserved on round-trip (`ADR 0013`) rather than compiled to bytecode | Complex scripted games or interactive apps relying on full ECMAScript/AS3 standard libraries cannot run arbitrary class code inside Test Movie | 40 h |
| 2 | 2.5D perspective transform (`3D Rotation` / `3D Translation`) | Elements use 2D `Affine` transforms (`crates/geom`); imported XFL `matrix3D` attributes are reported as skipped | Postcard-in-space 2.5D layer/symbol tilts must be approximated with 2D skew/scale or camera moves | 16 h |
| 3 | Phoneme-level neural/dictionary forced alignment for Lip-Sync | `crates/doc/src/lipsync.rs` uses pure, offline acoustic band/formant/ZCR/onset classification without external ML weights or language-specific phonetic transcripts | Fast dialogue in noisy recordings is less accurate than transcript-guided forced alignment | 12 h |
| 4 | Embedded video layers (`.mp4` / `.flv` playback on stage) | `crates/xfl/src/video.rs` imports video placeholders and metadata, and `file.export_video` renders output via `ffmpeg`, but timeline reference video scrubbing is not decoded frame-by-frame on stage | Animating directly over live-action reference video requires importing a PNG image sequence instead of a container video | 14 h |
| 5 | Layer parenting hierarchy & layer depth (`Z`-depth parallax) | Folder, Mask, Guide, Motion Guide, and Bone armatures are supported; direct layer-to-layer transform parenting and per-layer Z-depth camera parallax are not yet exposed | Cutout rigs that prefer layer parenting over Bone armatures or nested symbols must group parts in symbols | 10 h |
| 6 | Extended built-in CJK / RTL locale catalogs | English (`en`) and Korean (`ko`) ship built-in without external `CRAFT_I18N_DIR`; RTL layout (`ar`, `he`) awaits bidirectional text shaping in `egui` | Out-of-the-box Japanese/Chinese/RTL translations without an external `craft-i18n` checkout | 8 h |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-14 | major | Initial parity audit + closed 3 major gaps: Auto Lip-Sync (`anicraft_doc::lipsync`, `sound.auto_lip_sync`, Properties & Modify › Timeline UI), XFL/`.fla` export (`anicraft_xfl::export`, `file.export_xfl`), and built-in Korean (`ko`, `한국어`) UI localization (`src/i18n/ko.tsv` + Hangul font fallback) |
