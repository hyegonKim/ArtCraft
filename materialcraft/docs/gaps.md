# Gaps

> **Last reviewed:** 2026-11-14 · **Last updated:** 2026-11-14 · **Change:** major (procedural noise/pattern/normal/slope-blur nodes, portable preset JSON I/O, and Korean localization added) · **Target:** the professional node-based material workflow

## Current Capability Summary

- **Procedural Node Graph & Cooking Engine (`materialcraft-core`, `materialcraft-nodes`, `materialcraft-eval`)**:
  - 37 native catalog nodes across atomic nodes, procedural texture generators (`Perlin noise`, `Voronoi`, `Grunge`, `Brick`, `Tile sampler`), normal/height filters (`Normal`, `Curvature`, `Ambient occlusion`), blurs & warps (`Blur`, `Directional blur`, `Slope blur`, `Warp`, `Directional warp`), adjustments (`Levels`, `Curve`, `HSL`, `Gradient map`, `Dynamic gradient`, `Grayscale conversion`), blending & channels (`Blend`, `Channels shuffle`, `Distance`, `Emboss`, `Sharpen`, `Transformation`), vector/text (`SVG`, `Text`), `FX-Map` quadrant/iterate pattern graphs, and `Pixel Processor` / `Value Processor` function graphs.
  - 171 bundled reusable library function graphs across `Functions/*` sub-categories.
  - Full `.sbs` XML read/write round-tripping for all 37 native nodes, exposed parameter bindings, function graphs, and embedded resources.
- **Presets & Portable Material Archives (`materialcraft-formats`, `materialcraft-engine`, `materialcraft-automation`)**:
  - Graph parameter presets (`graph.preset.save`, `graph.preset.apply`, `graph.preset.delete`) plus portable preset JSON export/import (`graph.preset.exportJson`, `graph.preset.importJson`, and Properties panel buttons).
  - Portable `.mcmat` archive export, MaterialX `.mtlx` export, multi-output texture export (`PNG`, `TGA`, `TIFF`, `JPEG`, `WebP`, `OpenEXR`), TextureCraft shelf handoff, and workspace-scoped `material_export` MCP tool.
- **2D / 3D PBR Preview & Desktop/Web UI (`materialcraft-render`, `materialcraft-ui-egui`)**:
  - Interactive 2D tiled/channel view and 3D PBR preview with height parallax/displacement, explicit OpenGL/DirectX normal convention, HDRI lighting presets, and custom mesh loading.
  - Built-in UI localization in English (`en`), Arabic (`ar`, with RTL layout), and Korean (`ko`, `한국어`, with Hangul system font fallback).

## Remaining Gaps

Every known shortfall against the professional node-based material workflow, most important first.

| # | Gap | Evidence | Impact | Estimate | Parity doc |
|---:|---|---|---|---:|---|
| 1 | Extended library breadth (splines, scan processing, additional grunges) | 37 native nodes + 171 functions + FX-Map ops vs. ~400 library graphs in the reference library | Workflows that rely on spline/path tools, photogrammetry scan de-lighting, or specialized weathering graphs | 24 h | target-app-parity (Node library) |
| 2 | GPU 3D viewport with hardware tessellation and point/spot lights | CPU PBR renderer with height parallax/displacement | Real-time high-poly tessellation and multi-light rig editing | 12 h | target-app-parity (3D view) |
| 3 | Mesh-based generators and map bakers | Mesh maps can be bound as input previews, but mesh baking lives in TextureCraft | Starting a material graph directly from a high/low-poly mesh bake inside MaterialCraft | 15 h | target-app-parity (Node library) |
| 4 | Binary published-archive reader | Portable `.mcmat` archives and JSON presets cover authoring/TextureCraft handoff; proprietary binary archives are not decoded | Opening third-party compiled binary material archives directly | 20 h | target-app-parity (Files) |
| 5 | Function loops and cross-package dependency manager | Function graphs run per parameter/processor; linking external resources and dependency manager remain disabled in Explorer | Iterative function loops; managing external `.sbs` package links | 6 h | target-app-parity (Function graphs) |
| 6 | Input visibility expressions | `visibleIf` boolean expressions on exposed graph inputs are not evaluated yet | Conditional parameter hiding on large procedural materials | 4 h | target-app-parity (Graph parameters) |
| 7 | Keymap editor and additional locales | English, Arabic, and Korean UI catalogs ship; shortcuts are fixed in code | Custom shortcut remapping and additional European/Asian locales | 5 h | localization-parity |
| 8 | Graph minimap and multi-graph tabs | Single active graph canvas with navigation pins (`G`), frames, and comments | Side-by-side graph tab switching and bird's-eye minimap | 5 h | target-app-parity (Graph editing, UI and UX) |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-14 | major | Added 8 procedural generator/filter nodes (`perlin_noise`, `voronoi`, `grunge`, `brick`, `tile_sampler`, `curvature`, `ambient_occlusion`, `slope_blur`) with `.sbs` round-trip mappings, `graph.preset.exportJson` / `graph.preset.importJson` + UI buttons, Korean (`ko`) UI localization, and reconciled `material_export` MCP status |
| 2026-11-02 | minor | Portable `.mcmat` materials for TextureCraft closed the published-archive handoff gap |
| 2026-11-02 | major | Closed the function editor, dynamic parameters, FX-Map/Text/SVG, presets and groups, browser file I/O and accessible names; re-ranked |
| 2026-11-02 | major | First list |
