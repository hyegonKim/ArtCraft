# MaterialCraft roadmap

**Current focus (2026-11-08):** native primitive editing, reusable SBS functions and interoperable save/load.

The old Rust procedural library has been removed. The main catalog has 30 entries: 26 native
primitives and four graph input/output items. Functions have 85 operations; pattern graphs have
Quadrant, Switch, Iterate and Dot. See [the primitive workflow](docs/atomic-workflow.md) for usage and limits.

Earlier percentage estimates and node counts are superseded by this change. Reference-app
rendering parity and arbitrary SBS round trips have not been established.

## Next steps

1. Verify the original atomic sample in the reference application
2. Compare primitive outputs and parameter semantics against reference results
3. Validate the converted standard library in the reference application and establish redistribution rights
4. Extend the SBS library beyond reusable functions to processing graphs

## Progress log

| Date | What landed |
|---|---|
| 2026-11-08 | Explorer folder hierarchy and package/folder/graph context menus, native SBS empty folders and resource paths, undoable copy/move/rename/remove, and package reload/save-copy actions |
| 2026-11-08 | Portable `sbs://` function instances in the Library, preserved package folders and missing references, and a converted library retaining all 171 supplied function bodies (2,334 nodes); external runtime equivalence and redistribution rights remain unverified |
| 2026-11-02 | Cross-app materials: `.mcmat` portable materials, Send to TextureCraft, input-node preview images (mesh maps from TextureCraft), live re-cooks from TextureCraft through the CLI |
| 2026-11-02 | Visual function editor, dynamic parameters, presets and groups, 105 more nodes (flood fill, FX-Map, SVG, Text, 3D noises, weathering, material blending), mesh import, browser file I/O, 2D histogram, CLI `--set`, better samples |
| 2026-11-02 | First day: engine, ~120 nodes, cooking cache, packages, export, the full editor UI, 3D preview, CLI, control channel with synthetic input, MCP server, wasm builds |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-02 | minor | ≈ 66% → ≈ 67% (portable materials) |
| 2026-11-02 | major | ≈ 51% → ≈ 66% |
| 2026-11-02 | major | First version |
