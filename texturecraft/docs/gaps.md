# Feature Parity & Gaps

Clean-room parity audit of **TextureCraft** (`v0.6.1`) against the professional 3D model texture-painting workflow.

## Implemented Core Capabilities

| Area | Status | Implementation |
|---|---|---|
| 3D Mesh Import & Export | Complete | Wavefront `.obj` and `.gltf` / `.glb` import with MikkTSpace tangent generation, multi-material texture sets, UDIM / UV-tile workflows, mesh reimport (`file.reimportMesh`) and `.obj` / `.glb` export (`crates/formats/`, `crates/geom/`). |
| PBR Channels & Extended Lobes | Complete | Base Color, Metallic, Roughness, Normal (OpenGL / DirectX, detail + RNM combine), Height, Ambient Occlusion, Emissive, Opacity, Specular/Glossiness, Anisotropy, Clear Coat, Sheen, Transmission, and Subsurface Scattering (`crates/core/src/channel.rs`, `crates/compose/`). |
| Non-Destructive Layer Stack | Complete | Paint layers, Fill layers (uniform, 18 procedural 3D/2D noises, image, linked `.mcmat`/`.mcraft` procedural packages), Pass-through & blended Folders, Linked Layer Instances across texture sets, Height-based masking, Geometry masks, Anchor points, Color Selection masks, Compare masks, Levels, and 22 Mask/Content Filters (`crates/core/src/layer.rs`, `crates/compose/src/eval.rs`). |
| Mesh Map Baker (CPU + High-Poly Raycast) | Complete | SAH BVH raytracer baking Tangent Normal, World-Space Normal, ID, Ambient Occlusion, Multi-Radius Curvature, Position, Thickness, Height, Bent Normals, and Opacity with cage, frontal/rear distance, by-mesh-name matching, diffusion padding, and antialiasing (`crates/bake/`). |
| Mesh-Map Mask Generators | Complete | 9 built-in generators: `EdgeWear`, `Dirt`, `ThinAreas`, `Curvature` (7-band multi-radius detail weights), `AmbientOcclusion`, `Position`, `Direction`, **`WorldSpaceNormal`** (high-poly baked world normal directional mask with fallback), and **`Cavity`** (combined multi-radius concave curvature + AO pocket crevice mask), with full X/Y/Z direction and detail-size controls in the Properties dock (`crates/core/src/layer.rs`, `crates/compose/src/eval.rs`, `crates/ui-egui/src/dock.rs`). |
| Procedural Smart Masks & Smart Materials | Complete | 27 built-in Smart Masks (including `World Normal Top`, `Crevice Grime`, and `Oxide Bloom`) and 65+ physically calibrated Materials & multi-layer Smart Materials (including `Steel Frost Coated` and `Titanium Crevice Worn`), plus `.tcmat` file save/load and portable JSON envelope export/import (`material.exportJson` / `material.importJson`) (`crates/engine/src/{smart_masks.rs, materials.rs, material_lib.rs}`). |
| PBR Brush, Projection & Stencil Engine | Complete | 3D/2D tangent-aligned stamp painting, Eraser, Projection & Stencil painting, Clone stamp, Smudge, Polygon/Mesh/UV-chunk fill, Gradient tool, Bezier Path tool with live stroke re-evaluation, Geometry decals, Lazy mouse, Symmetry, and Brush presets (`crates/paint/`, `crates/engine/src/path_tool.rs`). |
| Texture Set & MaterialX Export | Complete | Channel-packed export templates (glTF Metallic-Roughness, PBR Metal/Rough, Specular/Glossiness, Unreal packed ORM, Unity MaskMap, custom templates), PNG / JPEG / TGA / TIFF / OpenEXR 8/16/32-bit output, dilation padding, and MaterialX `.mtlx` interchange (`crates/formats/src/export/`). |
| Viewport & Path-Traced Renderer | Complete | Split 3D/2D wgpu viewport with split-sum GGX IBL, procedural HDR environments, solo channel/mesh-map inspection, Quick Mask, and progressive path-traced rendering mode (`crates/render/`). |
| Localization (`ko` Korean UI) | Complete | Built-in Korean (`ko`, `한국어`) UI translation catalog (`crates/ui-egui/src/i18n_ko.tsv` + `crates/ui-egui/src/i18n.rs`) paired with cross-platform Hangul fallback font loading (`crates/ui-egui/src/fonts.rs`). |

## Remaining Intentional Non-Goals & Future Work

- **Proprietary Project / Archive Binary Formats**: By clean-room design, proprietary third-party project, smart-material, and compiled-archive binaries are non-goals; TextureCraft uses open `.tcraft` / `.tcmat` archives, portable JSON envelopes, `.mcmat` / `.mcraft` procedural links, glTF 2.0, and MaterialX.
- **Particle / Physics-Simulated Weathering Brushes**: Rain/drip/fracture particle brushes simulated on the 3D surface remain a future milestone; static leak/streak/cavity smart masks cover common weathering workflows today.
- **Python / JS In-Process Scripting**: Automation is provided cleanly out-of-process via the headless CLI, JSON-lines control socket, and STDIO MCP server (`crates/automation/`) rather than an embedded scripting VM.
