# Architecture

> **Last reviewed:** 2026-11-05 · **Last updated:** 2026-11-05 · **Change:** evaluator and editor organization · **Target:** the professional node-based material workflow

MaterialCraft is a Cargo workspace of small crates in strict layers (`cargo xtask layers` enforces
them). Everything below the UI is plain Rust with no UI toolkit and builds for
`wasm32-unknown-unknown`.

```text
L0  core      document model: Package → Graph → Node / Connection / Frame / Comment / Pin / ExposedParameter;
              Value (float, vectors, int, bool, color, enum, text, curve, gradient, function graph)
L0  image     Image: grayscale or RGBA f32 planes behind an Arc (copy-on-write, free to share);
              wrap/clamp sampling, bilinear, box/bilinear resampling, sRGB, 8-bit display
L0  geom      meshes, primitives (rounded cube, sphere, cylinder, plane, torus, material ball), BVH, camera
L1  nodes     30 main-graph entries (NodeDefinition), 85 function operations, four pattern node kinds;
              shared image operations and iterative function/pattern execution
L2  eval      cooking: topological order, output sizes (relative to input / parent, absolute, preview
              cap), parameter resolution (defaults, overrides, bindings to exposed inputs), the
              content-addressed LRU cache, graph instances (sub-graphs, recursion-guarded)
L2  formats   native .sbs packages, .mcraft ZIP packages, .mcmat portable materials, bitmap/SVG decode,
              texture encode (PNG/JPEG/TGA/BMP/WEBP/EXR/HDR), atomic writes
L3  engine    Session (open packages, active graph, selection, 2D/3D view state, snapshot undo, cache),
              the command registry, inspect, sample materials and graph templates
L3  render    the 3D view: BVH ray casts, the height output displacing a subdivided mesh, GGX PBR with
              prefiltered IBL + SH diffuse, alpha transparency (layered ray compositing) and an
              alpha channel view, ACES tone mapping; procedural environments
L4  automation MCP server (rmcp), JSON-lines RPC (stdio/TCP with token), bridge client, workspace sandbox
L5  ui-egui   the egui shell: docking (craft-ui), graph editor, library, properties, 2D/3D views,
              dialogs, menus, control channel with synthetic input
apps          materialcraft (desktop), materialcraft-cli, materialcraft-web
```

## Cooking

A node's cook is a pure function `fn(&CookContext) -> Result<Vec<Image>, String>`: `CookContext` carries the output
size, the seed, tiling, the inputs (by port order, `None` when unconnected), the resolved parameters,
a resource decoder and the graph's exposed values. `eval::cook` walks the graph in dependency order.
For each node it hashes the definition id, resolved parameters, size, seed, tiling, the keys of its
input images, resource bytes and (for instances) the sub-graph: equal keys hit the cache, so an edit
re-cooks only the nodes downstream of it, and undo finds earlier results again. A failing node shows
a magenta image and its error; nodes after it still cook.

Each cook builds a borrowed `core::GraphIndex` for node and connection lookups. `PreparedGraph`
resolves the needed nodes' ports once; `graph_execution` schedules them and `node_execution` handles
a single node. Parameter resolution, graph instances, value execution, output collection and cache
keys have separate modules. The index lives only for the cook, so it needs no invalidation protocol.

Function execution shares one interpreter. Pixel Processor borrows the caller's variables and keeps
only assignments in its per-pixel state; ordinary function evaluation retains the full environment
for callers such as FX-Map that read the resulting variables.

Sizes: generators take the graph ("parent") size; filters take their main input's size; both can be
offset by powers of two or set absolutely. While editing, outputs are capped at the preview size
(512 px by default), with pixel-based parameters scaled by `w / 256` so previews and full-size
exports look the same. Exports cook at full size through the same cache.

The 2D toolbar and the 3D view's Display menu expose the shared preview limit (256, 512, 1024,
2048 or 4096 px, or Full size). This is a cap, so a 4K preview requires a graph/node configured
for 4096 px or larger. Both controls run `view.previewSize` and persist the choice in preferences;
control-channel calls to the same command also retain their size across UI frames. The 3D view
uses those cooked maps, while its rendered frame follows the panel's physical pixel dimensions.
If the device supports smaller textures than the cooked image, the 2D view reduces only the
uploaded texture to that limit; source pixels, readout and screenshots retain the cooked size.

The desktop UI cooks on a worker thread (`ui-egui/src/cook.rs`) from a snapshot of the package and
keeps drawing the last results; the browser cooks on the UI thread.

## Commands and automation

`Session` exposes the active package and keeps inactive editing state in `OpenDocument`. Switching
packages retains their undo histories, selections and previews and invalidates pending cook results.
Linked main and Function graph instances store a source path, graph name and cached connector interface.
`linked_packages` publishes source snapshots and refreshes dependent packages; `linked_files`
resolves saved references on native open. Source snapshots share a flat registry of `Arc<Package>`
values whose own registries are empty, so cross-file references cannot create ownership cycles.
The registry is runtime state; SBS serializes the file references and connectors instead.
Function instances resolve to bounded runtime call bodies before cooking. Each compiled call owns
its program and reuses separate evaluation storage, so source lookup and compilation never happen
per pixel. Calls share the execution-step budget and are capped at eight levels and 4096 nodes.
SBS Function nodes store the source URI in their instance data; a `materialcraftInterface` cache
preserves unconnected input pins when a source file is unavailable.

Package folders are part of each graph's SBS identity. Exact paths take priority over leaf names;
a leaf name is accepted only when it is unambiguous. Unconnected function-instance inputs use
the source's declared defaults. Unknown function operations retain their connector names, data
and output designation so an unavailable implementation does not erase the containing graph.
Missing external resources retain their paths for repair on a later load.

Empty folders also persist as native SBS groups. Explorer renders these groups as a collapsible
tree, including resources in their original folders. Folder creation, rename, removal, copying
and moving use engine commands with snapshot undo. Renames and moves update local instance paths;
copies retarget references between copied graphs. The Explorer clipboard survives document changes.
Single clicks select Explorer rows; double clicks open their graph or input/output preview.
The Explorer New submenu creates items immediately with numbered default names, advancing past
name conflicts in the destination folder. Rename remains an explicit context-menu action.

`sbs://` resolves beneath `assets/sbs/`. Native installations can place that directory beside the
executable or set `MATERIALCRAFT_SBS_DIR`; packaged and browser builds also embed the standard
library. Alias paths cannot escape that root, and saved packages retain the alias. The Function
Library lists these source graphs and inserts references into Function and processor canvases.
Library aliases are read-only save destinations; an edited library can be saved to a local path.
Resource paths resolve beside their owning package: `resources/tile.png` in
`sbs://functions.sbs` reads `assets/sbs/resources/tile.png`, also addressable as
`sbs://resources/tile.png`. Nested packages use their own adjacent `resources/` directory.
Loaded resource bytes are embedded on save; unavailable files keep their reference and missing state.

Every session loads the whole library folder once per process (`engine::node_library`, cached
until `library.reload`). Each package joins every session's dependencies, so its graphs can be
instanced, and the Library lists them: function graphs under Functions, sorted into its
sub-categories by their folder (`Functions/Math/Pi` → Constant), other graphs under their attribute
category. A package whose titled frames name Library categories ("Filters - Blurs") can also lay
the tree out (`formats::sbs::read_layout`): each node inside a frame becomes an entry there, listed
as missing until a package in the folder provides it. Installed and browser builds embed
`functions.sbs`.

Importing a user-supplied function package preserves its graph bodies, connections and positions;
it does not replace them with the bundled library. Redundant legacy instance connectors are retained
on save and evaluated only through the declared inputs. Conflicting obsolete connections remain
visible errors. A shared typed system-variable catalog drives Get-node suggestions and execution.
Ordinary functions expose size, log2 size, pixel ratio, tiling, physical size, UV tile, time and
normal-format variables. Pixel Processor adds `$pos`; FX-Map adds `$pos`, `$depth`, `$depthpow2`
and `$number`. Reusable function instances inherit their caller's scope. Out-of-scope variables
remain undefined even if a user input has the same name, and Set cannot overwrite system variables.
Get properties offer a type-filtered dropdown and accept typed custom names.

Node tiling and pixel ratio resolve from absolute values, the first connected image input or the
parent graph. SBS preserves these inheritance modes and the graph's `physicalSize` attribute.
The host supplies time, UV tile and a raw normal-format integer through `EvaluationEnvironment`;
the static authoring defaults are zero. These values propagate through nested graph instances
without becoming constants in saved functions. Relevant variable changes invalidate cached cooks.
The normal-format integer is passed through without assigning a convention label to an unverified code.

Numeric, Boolean, color, enum and text value parameters can own a Function graph, including
inputs on material graph instances. The six base parameters store their functions separately from
node-specific parameters so native identifiers cannot collide. Size and seed functions establish
the evaluation context before other parameters run; downstream nodes inherit evaluated nominal
sizes rather than capped previews. SBS writes these functions as native dynamic values, including
incomplete graphs. Exposing a parameter creates a typed Get-node function. Properties offers the
same function creation, editing and reset actions for base parameters and instance inputs.
Only Specific Parameters offer exposure: a modal drafts the identifier, label, group, description,
editor, default, limits, clamp and step before one undoable command creates the input and function.
Inheritance buttons show distinct icons for input, parent and absolute modes.

The local `assets/sbs/functions.sbs` is converted from a user-supplied package with
`cargo run -p materialcraft-formats --release --example standard_function_library -- <source.sbs> assets/sbs/functions.sbs`.
The converter preserves all 171 function bodies (2,334 nodes), folder paths, identifiers, inputs,
defaults and internal references. It writes new interface-based descriptions, generates node
positions and omits source parameter descriptions, annotations and icons. It rejects import warnings and functions that
cannot compile instead of substituting empty graphs. The source graph bodies are retained;
rewritten metadata does not establish redistribution rights.
Local tests compare every converted body and evaluate all 171 functions against the supplied
package with valid inputs and caller context. External runtime import and numerical compatibility
still require validation. SBS's `updaterVersion` derives from the workspace's Cargo version;
its format schema version is a separate constant.

Every user-visible action is a `CommandSpec` (`engine/src/*_cmds.rs`): id, label, menu, shortcut,
params doc, `enabled`, `run`. The UI calls them through `MaterialCraftApp::run`; the CLI, the JSON
control channel and MCP dispatch the same ids. `Session::edit` snapshots the package before running
an edit and restores it on error; `edit_merged` folds repeated edits with the same key (a slider
drag) into one undo step. See [control-protocol.md](control-protocol.md).

## The UI

`MaterialCraftApp` owns the session and `UiState` (serde, persisted by eframe, readable and writable
through `ui.inspect` / `ui.set`). The dock (craft-ui) hosts eight panels around the protected graph
panel. The graph editor lays nodes out in graph space, draws cards with thumbnails from the cook
results (uploaded once per cache key), wires as cubic beziers between typed ports (hit-tested
against the curve, so wires can be selected, deleted, swapped or split with a reroute dot), and turns
gestures into commands only when they complete. See [wires-and-alpha.md](wires-and-alpha.md). The 3D view renders with `materialcraft-render`
on the CPU: half resolution while the camera moves, 2×2 supersampled when it rests.

The 3D material has its own `MaterialPreview` assignment state. Automatic graph output usages and
explicit `MaterialSource` overrides resolve to channels from `core::MaterialChannel`, shared with the
renderer and menus. A source records its graph, node and named port; selection and 2D preview do not
change it. `PreviewCook` publishes the editor cook and any other assigned graphs together through
the existing worker/cache, so opening a Function Graph preserves the material. Render invalidation
includes both source assignments and cooked content. Snapshot undo and graph deletion retain or
adjust source ownership; package replacement clears old assignments.

`core::ComponentUsage` defines output metadata once for node properties, SBS and exports. Its
indices stay stable as choices are added; renderable channels remain a separate capability.
Explorer and canvas node labels share display formatting without rewriting identifiers. Explorer
lists every graph input/output, and graph drops dispatch the existing material assignment command.

`crates/ui-egui/tests/snapshot.rs` renders the real UI offscreen with egui_kittest + wgpu for visual
checks and README screenshots.

The regular and function canvases separate drawing, geometry, navigation and input handling.
Shared modules own node cards, selection and annotation gestures. The regular canvas borrows the
document while drawing, then dispatches edits. Graph Properties follows the same pattern; its
`property/` modules separate panel routing, node and graph settings, parameter controls, curves
and gradients. Function and pattern Properties dispatch value edits without cloning graphs in the UI.
Explorer inspection uses a document-and-graph target independent of the canvas. Shared graph
attribute controls retain text drafts until focus changes, then dispatch targeted engine commands.
Inactive packages record those edits in their own snapshot history. Both graph kinds share the SBS
attribute codec; display labels are resolved separately from reference identifiers.

The properties panel builds every parameter editor from the node catalog; the Levels node adds a
histogram editor with range handles on top of its sliders ([levels-editor.md](levels-editor.md)).

### Shared graph editing

The graph panel renders one toolbar before dispatching to the material, Function Graph or FX-Map
canvas. Framing, alignment, distribution and snapping occupy the same positions. Image inserts,
parent texture size, annotations and parent-graph navigation appear only where the graph supports
them. Alignment uses the same engine command for all three models, with explicit embedded node IDs
and an explicit Function target or FX-Map owner ID, so overlapping IDs cannot move the parent material nodes.
Pixel Processor, Value Processor and parameter functions use that same main panel and function node
catalog, with one target-aware availability rule shared by node creation, Library and Add node.
Both processor types accept Sample gray and Sample color to read their image inputs.
A breadcrumb and parent action return from nested functions, FX-Maps and local graph instances
while retaining the parent view and material preview.

`ui-egui/graph/graph_camera.rs` owns framing, pointer-centered scroll/pinch zoom and navigation
shortcuts. `node_selection.rs` owns click and box selection; `port_hit.rs` owns nearest-port picking;
`wire_geometry.rs` owns curve construction. Node movement uses the engine's `graph_grid::translate`
for both previews and commits.

`engine/graph_edit/` owns Function/FX-Map selection commands, clipboard remapping, deletion and
connected-node traversal. Its small model adapters supply node fields, link direction, ID allocation
and result/root handling. Material clipboard commands retain their named-port connections and
cross-graph clipboard rules. Rendering and connection validation remain specific to each model;
all document changes still pass through engine commands and snapshot undo. `selection_commands.rs`
routes the existing selection and clipboard command IDs to these adapters, including headless calls.
Read-only selection actions borrow the graph. Document, selection and clipboard updates are published
only after validation succeeds; clipboard provenance prevents external links crossing graph targets.
`graph_validation.rs` supplies the same checks to creation, legacy setters and targeted edits.

`graph/editor_target.rs` resolves command ownership for the material canvas, standalone Function
Graphs, FX-Map canvases and embedded Function Graphs. Menus, keyboard shortcuts and
`ui.menu.invoke`, Library and Properties use that resolver; `ui.inspect` reports the same editor and
its canvas coordinates. Embedded editors record their owning graph and close when undo or another
command makes that owner unavailable.
Common context-menu entries live in `graph/edit_menu.rs`. Pointer gestures finish only when their
initiating button is released; Escape or focus loss cancels pending canvas movement.

Function and FX-Map rendering borrow the document, prepare screen geometry, and dispatch edits
after painting. Function wires reuse one node-layout index instead of scanning the graph for each
endpoint. The engine's `graph.function.edit` and `graph.pattern.edit` commands own creation,
movement, connections, values and result/root changes. Function targets identify standalone,
parameter, dynamic-parameter or nested FX-Map functions explicitly. Value updates may request
`merge: true`; `edit.finish` closes that undo group. The UI closes groups at gesture boundaries,
while discrete edits always receive their own undo entry.


## Files

SBS XML is the document representation. A native `.sbs` saves assets in an adjacent `resources/`
directory and references them with relative paths. A `.mcraft` is a ZIP containing the fixed root
entry `package.sbs` and `resources/`, with no manifest or private graph payload. Both formats open,
save, reload and save copies independently. The same clean-room SBS codec writes native filters,
ports, parameters, function graphs, pattern graphs and resources. Unsupported imports produce
warnings; unknown nodes remain placeholders.

`formats::project::ProjectFiles` builds either representation without changing the live package.
Imports retain original bytes in snapshot history, including before the first save. Content-based
asset filenames avoid overwriting resources shared by another SBS and allow the XML to be replaced
last. Missing assets cause saving to fail. Existing unreferenced files are left in place. Archives
are read in memory and validate paths, duplicate entries, symlinks and malformed data. Project I/O
and resource imports impose no fixed file-size, resource-count or linked-package-count ceilings.
Referenced packages are included under `resources/packages/`;
unresolved installed-library aliases retain their `sbs://` references. Autosaves use `.mcraft`.

Explorer shows its reserved Resources folder once it contains assets or subfolders. Its Import
submenu accepts bitmaps or SVG vector graphics into the selected asset folder. Subfolders can be
created, renamed and removed; graph creation and graph-folder moves into this tree are rejected.
The root Resources name is reserved regardless of capitalization and cannot be renamed, copied, moved or
removed. Import creates an asset; dragging it onto the canvas or choosing Add to graph creates the
appropriate bitmap or SVG node. Drops from another open package copy the asset into the destination
in one undo step. Imported native SBS groups retain their original resource placement for round-trip
fidelity. Folder Properties exposes Identifier and Description; descriptions persist in native SBS
group `desc` elements, including inside MCRAFT. The reserved root identifier is read-only.

Bitmap imports support DDS, WebP, GIF, HDR, EXR, JPEG, PGM, PNG, TIFF, Targa and BMP. DDS uses the
MIT OR Apache-2.0 `dds` decoder; GIF and ASCII/binary PGM use `image`. GIF loads its first frame;
DDS loads its first surface at the largest mip level. Each bitmap format has its own picker filter,
and vector import offers SVG only. Both pickers include All formats, restricted to their supported
extensions, and descriptive names with extension patterns. Open offers SBS XML Package and
MaterialCraft Project plus All formats; Save As offers those two formats separately.

The browser downloads `.mcraft` directly. SBS downloads use a transport ZIP containing the named
SBS and its adjacent resources; extract it to work with the native files. Both forms can be opened
in the browser. A bare SBS can load embedded assets, but browser file pickers cannot automatically
read adjacent files; use the SBS transport ZIP to include those assets.

The SBS `read` and `write` modules expose codec entry points. `SbsReader` and `SbsWriter` own
serialization state; focused helpers handle values, functions, patterns and canvas annotations.
Native value type codes come from `core::ValueType` rather than a second scalar type table.

See [atomic-workflow.md](atomic-workflow.md) for the tested scope and remaining limitations.

## Other apps

A `.mcmat` portable material (`formats/src/material/`, `engine/src/material_cmds.rs`) holds a
graph's cooked maps, its portable exposed inputs, presets and input nodes, and the source package.
TextureCraft paints with it and re-cooks it by running `materialcraft-cli run <package>
file.exportMaterial {path, size, values, inputs}`: the apps share the file format and the command
line, never code. Graphs keep preview images for their input nodes (`Graph::input_images`, mesh
maps from TextureCraft); `eval::cook` feeds them to the top-level graph, and a caller's images win.

Preview cooks publish the editor graph and assigned material graphs together. A synchronous cook
invalidates older worker results. Displacement uses the assigned Height source even while another
graph or an embedded function is open. Native image loading checks dimensions before decoding;
bitmap resources and graph input previews can request linear data-map reads.
