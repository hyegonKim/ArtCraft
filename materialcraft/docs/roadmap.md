# Roadmap detail

> **Last reviewed:** 2026-11-02 · **Last updated:** 2026-11-02 · **Change:** minor (portable materials done, ≈ 67%) · **Target:** the professional node-based material workflow

## Milestones

| Milestone | Status | Scope |
|---|---|---|
| M0 Skeleton | Done | Workspace, licences, AGENTS.md, xtask (layers, wasm, ci, version, ico, icns), CI and packaging |
| M1 Core + engine | Done | Document model, images, node library, cooking with cache, session, commands, inspect, samples |
| M2 UI shell | Done | Docked layout, graph editor, library, properties, 2D and 3D views, dialogs, menus, status bar |
| M3 Node breadth | In progress | 227 of ~400 library nodes |
| M4 Function graphs | In progress | Evaluator, visual editor and dynamic parameters done; loops and reusable functions next |
| M5 Graph features | In progress | Exposed inputs, instances, presets and groups done; visibility conditions next |
| M6 Files | In progress | Packages, bitmaps, export, mesh import, browser I/O and `.mcmat` portable materials (TextureCraft interop) done; package dependencies next |
| M7 Automation | Done | CLI, control channel with synthetic input, MCP server |
| M8 3D view depth | In progress | Mesh import done; GPU renderer, tessellation, lights next |
| M9 Web + packaging | In progress | WASM builds and browser file I/O done; the first release run next |
| M10 Polish | In progress | Home screen at startup and accessible names done; keymap editor, translations, minimap next |

## Current focus

1. Node library breadth (gap 1): scan processing, more grunges, spline tools, SDF shapes.
2. The GPU 3D view (gap 2) and mesh-based generators (gap 3).
3. Function loops (gap 5) and the portable-material MCP tool (gap 4).

## Remaining effort (Opus 5.5 agent hours)

| Work | Estimate |
|---|---:|
| Node library to ~400 nodes | 30 h |
| 3D view on the GPU, tessellation, lights | 12 h |
| Mesh-based generators | 15 h |
| Portable materials over MCP | 1 h |
| Function loops and reusable functions | 5 h |
| Graph features: visibility conditions, per-instance sizes | 4 h |
| UX: keymap, translations, minimap, tabs | 13 h |
| Release | 3 h |
| **Total** | **≈ 83 h** |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-02 | major | ≈ 51% → ≈ 66%; re-estimated |
| 2026-11-02 | major | First version |
