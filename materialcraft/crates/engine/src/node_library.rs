//! The node library. Every SBS package in the library folder (`assets/sbs`, or the bundled copies)
//! is loaded once per process and lends its graphs: function graphs go under Functions (sorted by
//! their classification tags, with folder fallbacks), other graphs under their attribute category. Native nodes, function operations,
//! FX-Map nodes and graph items fill the rest of the tree. A package whose titled frames are library
//! categories ("Filters - Blurs") can also lay the library out: each node inside a frame becomes an
//! entry in that category, listed as missing until a package provides it.

mod library_canvas;
mod library_entry;
mod library_item;
mod library_placement;
pub use library_canvas::LibraryCanvas;
pub use library_entry::LibraryEntry;
pub use library_item::LibraryItem;
use library_placement::LibraryPlacement;

use materialcraft_core::{Package, PatternNodeKind};
use materialcraft_formats::sbs::{LayoutItem, library};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// The library tree in display order: top-level categories and their sub-categories. The UI adds
/// Favorites above it
pub const CATEGORIES: &[(&str, &[&str])] = &[
    ("Graph items", &[]),
    ("Atomic nodes", &[]),
    ("FxMap nodes", &[]),
    ("Function nodes", &["Constant", "Vector", "Variables", "Samplers", "Cast", "Operator", "Logical", "Comparison", "Function", "Control"]),
    ("Texture generators", &["Noises", "Patterns"]),
    ("Filters", &["Adjustments", "Blending", "Blurs", "Channels", "Effects", "Normal map", "Tiling", "Transforms"]),
    ("Material filters", &["1-click", "Effects", "Transforms", "Blending", "PBR utilities", "Scan processing"]),
    ("Mesh-based generators", &["Mask generators", "Weathering", "Utilities"]),
    ("Spline & Path tools", &["Spline tools", "Path tools"]),
    ("MCF functions", &["Primitives", "Operators", "Transforms"]),
    ("Functions", &["Color", "Comparison", "Constant", "Conversion", "Maths", "Parity", "Random", "Transformation", "Various", "Easings"]),
    ("3D view", &["HDRI environments", "HDRI tools"]),
    ("PBR materials", &["Bricks", "Concrete", "Fabric", "Ground", "Metal", "Paper", "Pavement", "Tiles", "Wood", "Stone"]),
    ("Values", &["Constant"]),
];

/// Older layouts name categories MaterialCraft has renamed: (old, current)
const RENAMED: &[(&str, &str)] = &[("SDF functions", "MCF functions")];

/// Words that place a material in a sub-category when its layout frame names only the top level
const SUB_KEYWORDS: &[(&str, &str, &[&str])] = &[
    ("PBR materials", "Bricks", &["brick"]),
    ("PBR materials", "Concrete", &["concrete", "cement"]),
    ("PBR materials", "Fabric", &["fabric", "cloth", "textile", "leather"]),
    ("PBR materials", "Ground", &["ground", "gravel", "soil", "sand", "dirt", "grass", "mud"]),
    ("PBR materials", "Metal", &["metal", "steel", "iron", "copper", "rust"]),
    ("PBR materials", "Paper", &["paper", "cardboard"]),
    ("PBR materials", "Pavement", &["pavement", "paving", "cobble", "asphalt"]),
    ("PBR materials", "Tiles", &["tile"]),
    ("PBR materials", "Wood", &["wood", "plank", "bark"]),
    ("PBR materials", "Stone", &["stone", "rock", "marble", "granite"]),
];

/// Everything loaded from the library folder
#[derive(Debug, Default)]
pub struct NodeLibrary {
    /// Library packages (layouts excluded) and their own dependencies, by `sbs://` path
    pub packages: BTreeMap<String, Arc<Package>>,
    /// The `sbs://` files that laid the library out
    pub layouts: Vec<String>,
    pub warnings: Vec<String>,
    placements: Vec<LibraryPlacement>,
}

static SHARED: Mutex<Option<Arc<NodeLibrary>>> = Mutex::new(None);

impl NodeLibrary {
    /// The process's library, loaded on first use
    pub fn shared() -> Arc<NodeLibrary> {
        let mut shared = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        shared.get_or_insert_with(|| Arc::new(Self::load(&library::files()))).clone()
    }

    /// Read the library folder again (after packages were added or changed on disk)
    pub fn reload() -> Arc<NodeLibrary> {
        let library = Arc::new(Self::load(&library::files()));
        *SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(library.clone());
        library
    }

    /// Load `files` (`sbs://` paths)
    pub fn load(files: &[String]) -> NodeLibrary {
        let mut out = NodeLibrary::default();
        for uri in files {
            let bytes = match library::read(uri) {
                Ok(bytes) => bytes,
                Err(error) => {
                    out.warnings.push(format!("Library package {uri}: {error}"));
                    continue;
                }
            };

            // A layout's frames name library categories; anything else is a package of graphs
            let placements = match materialcraft_formats::sbs::read_layout(&bytes, uri) {
                Ok(frames) => placements(&frames),
                Err(error) => {
                    out.warnings.push(format!("Library package {uri}: {error}"));
                    continue;
                }
            };
            if !placements.is_empty() {
                out.placements.extend(placements);
                out.layouts.push(uri.clone());
                continue;
            }

            match materialcraft_formats::sbs::read_sbs(&bytes, uri, &crate::file_cmds::linked_files(uri)) {
                Ok(mut read) => {
                    crate::linked_files::normalize_references(&mut read.pkg, uri);
                    read.warnings.extend(crate::linked_files::load(&mut read.pkg));
                    out.warnings.extend(read.warnings);
                    for (path, package) in std::mem::take(&mut read.pkg.dependencies) {
                        out.packages.entry(path).or_insert(package);
                    }
                    out.packages.insert(uri.clone(), Arc::new(read.pkg));
                }
                Err(error) => out.warnings.push(format!("Library package {uri}: {error}")),
            }
        }
        out
    }

    /// Every entry, resolved against `dependencies` (a session's loaded packages): layout
    /// placements first, then graph items and native nodes no layout placed, then graphs of
    /// `sbs://` packages no layout placed
    pub fn entries(&self, dependencies: &BTreeMap<String, Arc<Package>>) -> Vec<LibraryEntry> {
        let mut out = Vec::new();
        let mut ids = BTreeSet::new();
        let mut placed_graphs = BTreeSet::new();
        let mut push = |out: &mut Vec<LibraryEntry>, mut entry: LibraryEntry| {
            // Reroute nodes are placed through the shared Graph items entry
            if matches!(&entry.item, LibraryItem::Operation { operation } if operation == "passthrough")
                || matches!(&entry.item, LibraryItem::Pattern { kind: PatternNodeKind::Dot })
                || matches!(&entry.item, LibraryItem::Node { def } if def == "dot" && entry.category != "Graph items")
            {
                return;
            }
            // The same node may sit in two categories; the id keeps each placement apart
            if !ids.insert(entry.id.clone()) {
                entry.id = format!("{}@{}", entry.id, entry.category);
                if !ids.insert(entry.id.clone()) {
                    return;
                }
            }
            out.push(entry);
        };

        for (item, category) in [(LibraryItem::Comment, "Comment"), (LibraryItem::Frame, "Frame"), (LibraryItem::Pin, "Pin")] {
            let id = format!("item:{}", category.to_ascii_lowercase());
            push(&mut out, entry(id, category, "Graph items", "", item, None));
        }
        push(&mut out, entry("node:dot".into(), "Dot node", "Graph items", "A reroute point for wires.", LibraryItem::Node { def: "dot".into() }, None));

        for placement in &self.placements {
            if let LayoutItem::Instance { package, graph, .. } | LayoutItem::FunctionInstance { package, graph } = &placement.item {
                placed_graphs.insert((package.clone(), graph.clone()));
            }
            if let Some(entry) = resolve(placement, dependencies) {
                push(&mut out, entry);
            }
        }

        let listed: BTreeSet<String> = out.iter().map(|entry| entry.id.clone()).collect();
        for definition in materialcraft_nodes::catalog() {
            let id = format!("node:{}", definition.id);
            if definition.id == "graph_instance" || listed.contains(&id) {
                continue;
            }

            let item = LibraryItem::Node { def: definition.id.into() };

            // Graph inputs and outputs sit with the atomic nodes; Graph items holds annotations and the Dot
            let category =
                if matches!(definition.id, "input_color" | "input_grayscale" | "input_value" | "output") { "Atomic nodes" } else { definition.category };
            push(&mut out, entry(id, definition.label, category, definition.description, item, None));
        }
        for operation in materialcraft_nodes::function::FUNCTIONS {
            let id = format!("function:{}", operation.id);
            if operation.id == "passthrough" || listed.contains(&id) {
                continue;
            }

            let category = category_path("Function nodes", Some(operation.category));
            push(&mut out, entry(id, operation.label, &category, "", LibraryItem::Operation { operation: operation.id.into() }, None));
        }
        for kind in PatternNodeKind::ALL.into_iter().filter(|kind| *kind != PatternNodeKind::Dot) {
            let id = format!("pattern:{}", kind.native_id());
            push(&mut out, entry(id, kind.label(), "FxMap nodes", "", LibraryItem::Pattern { kind }, None));
        }

        for (path, package) in dependencies.iter().filter(|(path, _)| path.starts_with("sbs://")) {
            for graph in &package.graphs {
                let name = graph.sbs_path();
                if placed_graphs.contains(&(path.clone(), name.clone())) {
                    continue;
                }

                if graph.function.is_some() {
                    for category in function_categories(graph) {
                        let placement = LibraryPlacement { category, item: LayoutItem::FunctionInstance { package: path.clone(), graph: name.clone() } };
                        if let Some(entry) = resolve(&placement, dependencies) {
                            push(&mut out, entry);
                        }
                    }
                    continue;
                }

                let placement = if graph.nodes.iter().any(|node| node.def == "output") {
                    let category = known_category(&graph.attributes.category).unwrap_or_else(|| "Other".into());
                    LibraryPlacement { category, item: LayoutItem::Instance { package: path.clone(), graph: name, outputs: Vec::new() } }
                } else {
                    continue;
                };

                if let Some(entry) = resolve(&placement, dependencies) {
                    push(&mut out, entry);
                }
            }
        }
        out
    }
}

/// Build an entry with a canvas derived from its item kind.
fn entry(id: String, label: &str, category: &str, description: &str, item: LibraryItem, missing: Option<String>) -> LibraryEntry {
    let canvas = item.canvas();
    LibraryEntry { id, label: label.into(), category: category.into(), description: description.into(), item, canvas, missing, warning: None }
}

/// A layout item as an entry, its label and availability taken from the loaded packages
fn resolve(placement: &LibraryPlacement, dependencies: &BTreeMap<String, Arc<Package>>) -> Option<LibraryEntry> {
    let category = placement.category.as_str();
    Some(match &placement.item {
        LayoutItem::Atomic { node: Some(def), .. } | LayoutItem::Node(def) => {
            let definition = materialcraft_nodes::find(def)?;
            entry(format!("node:{def}"), definition.label, category, definition.description, LibraryItem::Node { def: def.to_string() }, None)
        }
        LayoutItem::Atomic { filter, node: None } => {
            let item = LibraryItem::Node { def: filter.clone() };
            entry(format!("atomic:{filter}"), &humanize(filter), category, "", item, Some(format!("The atomic `{filter}` node is not in MaterialCraft yet")))
        }
        LayoutItem::Operation(operation) => {
            let definition = materialcraft_nodes::function::FUNCTIONS.iter().find(|definition| definition.id == operation);
            let label = definition.map_or_else(|| humanize(operation), |definition| definition.label.to_string());
            let missing = definition.is_none().then(|| format!("The `{operation}` function node is not in MaterialCraft yet"));
            entry(format!("function:{operation}"), &label, category, "", LibraryItem::Operation { operation: operation.clone() }, missing)
        }
        LayoutItem::Instance { package, graph, .. } | LayoutItem::FunctionInstance { package, graph } => {
            let function = matches!(placement.item, LayoutItem::FunctionInstance { .. });
            let source = dependencies.get(package).and_then(|source| source.graph_by_name(graph)).map(|(_, graph)| graph);
            let source = source.filter(|source| source.function.is_some() == function);
            let label = source.map_or_else(|| humanize(graph), |source| source.display_name().to_string());
            let description = source.map(|source| source.attributes.description.clone()).unwrap_or_default();
            let missing = source.is_none().then(|| format!("Not in MaterialCraft yet: needs {graph} from {package}"));
            let item = if function {
                LibraryItem::FunctionInstance { package: package.clone(), graph: graph.clone() }
            } else {
                LibraryItem::Instance { package: package.clone(), graph: graph.clone() }
            };
            let mut out = entry(format!("{package}#{graph}"), &label, category, &description, item, missing);
            out.category = sub_category(category, graph);
            out.warning = source
                .and_then(|source| source.function.as_ref())
                .filter(|function| function.output.is_none() || function.nodes.iter().any(|node| node.missing.is_some()))
                .map(|_| "This function uses nodes MaterialCraft does not have yet".to_string());
            out
        }

        // A bitmap anywhere but the 3D view stands for the atomic Bitmap node, not its image
        LayoutItem::Bitmap { .. } if !category.starts_with("3D view") => {
            let definition = materialcraft_nodes::find("bitmap")?;
            entry("node:bitmap".into(), definition.label, category, definition.description, LibraryItem::Node { def: "bitmap".into() }, None)
        }
        LayoutItem::Bitmap { resource } => {
            let name = resource.rsplit('/').next().unwrap_or(resource);
            let missing = Some(format!("Not in MaterialCraft yet: the {} image is not bundled", humanize(name)));
            entry(format!("bitmap:{resource}"), &humanize(name), category, "", LibraryItem::Bitmap { resource: resource.clone() }, missing)
        }
    })
}

/// The categories of a layout's frames, and what each places
fn placements(frames: &[materialcraft_formats::sbs::LayoutFrame]) -> Vec<LibraryPlacement> {
    let mut out = Vec::new();
    for frame in frames {
        let native = frame.items.iter().any(|item| matches!(item, LayoutItem::Operation(_)));
        let Some(category) = frame_category(&frame.title, frame.function, native) else { continue };
        out.extend(frame.items.iter().map(|item| LibraryPlacement { category: category.clone(), item: item.clone() }));
    }
    out
}

/// Letters and digits, lower case: "Spline &amp; Path Tools", "spline & path tools" and
/// "Spline-and-path tools" differ, but "1 click" and "1-click" match
fn key(text: &str) -> String {
    text.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// `Top/Sub` for a frame title. Graph frames read "Top - Sub" (or "Top"); function frames name
/// only the sub-category, under Function nodes when they hold native operations and Functions
/// otherwise
pub fn frame_category(title: &str, function: bool, native_operations: bool) -> Option<String> {
    if function {
        let top = if native_operations { "Function nodes" } else { "Functions" };
        let (_, subs) = CATEGORIES.iter().find(|(name, _)| *name == top)?;
        let sub = subs.iter().find(|sub| key(sub) == key(title))?;
        return Some(category_path(top, Some(sub)));
    }

    let (top, sub) = title.split_once(" - ").map_or((title, None), |(top, sub)| (top, Some(sub)));
    let top = RENAMED.iter().find(|(old, _)| key(old) == key(top)).map_or(top, |(_, current)| current);
    let (top, subs) = CATEGORIES.iter().find(|(name, _)| key(name) == key(top))?;
    match sub {
        None => Some((*top).to_string()),
        Some(sub) => subs.iter().find(|name| key(name) == key(sub)).map(|sub| category_path(top, Some(sub))),
    }
}

/// A category text from a graph's attributes, when it names a library category
fn known_category(text: &str) -> Option<String> {
    let mut parts = text.split('/');
    let top = parts.next()?;
    let (top, subs) = CATEGORIES.iter().find(|(name, _)| key(name) == key(top))?;

    // A sub-category the tree doesn't have names no category ("Functions/colorValues" is a folder)
    let sub = match parts.next() {
        None => None,
        Some(sub) => Some(*subs.iter().find(|name| key(name) == key(sub))?),
    };
    Some(category_path(top, sub))
}

/// Function tags select filters; explicit category paths and package paths remain fallbacks.
fn function_categories(graph: &materialcraft_core::Graph) -> Vec<String> {
    let subcategories = CATEGORIES.iter().find(|(name, _)| *name == "Functions").map_or(&[][..], |(_, subcategories)| *subcategories);
    let mut categories = Vec::new();
    for tag in graph.attributes.tags.split([',', ';']) {
        let tag = key(tag);
        let tag = if tag == "easing" { "easings" } else { tag.as_str() };
        if let Some(subcategory) = subcategories.iter().find(|subcategory| key(subcategory) == tag) {
            let category = category_path("Functions", Some(subcategory));
            if !categories.contains(&category) {
                categories.push(category);
            }
        }
    }
    if categories.is_empty() {
        categories.push(known_category(&graph.attributes.category).unwrap_or_else(|| function_category(&graph.sbs_path())));
    }
    categories
}

/// Join a top-level category with its optional subcategory.
fn category_path(top: &str, sub: Option<&str>) -> String {
    match sub {
        Some(sub) => format!("{top}/{sub}"),
        None => top.to_string(),
    }
}

/// The sub-category a graph's name suggests, for layouts that name only a top level
fn sub_category(category: &str, graph: &str) -> String {
    if category.contains('/') {
        return category.to_string();
    }

    let name = graph.rsplit('/').next().unwrap_or(graph).to_ascii_lowercase();
    SUB_KEYWORDS
        .iter()
        .find(|(top, _, words)| *top == category && words.iter().any(|word| name.contains(word)))
        .map_or_else(|| category.to_string(), |(top, sub, _)| category_path(top, Some(sub)))
}

/// Function graphs whose name decides their Functions sub-category
const FUNCTION_NAMES: &[(&str, &str)] = &[
    ("Pi", "Constant"),
    ("2Pi", "Constant"),
    ("deg_to_rad", "Conversion"),
    ("Degrees_to_Turn", "Conversion"),
    ("Curve_Function", "Various"),
    ("Roughness", "Various"),
];
/// Name prefixes that decide a function graph's sub-category
const FUNCTION_PREFIXES: &[(&str, &str)] = &[("Equality_", "Comparison"), ("NotEqual_", "Comparison")];
/// Library folders (below `Functions/`) and the Functions sub-category of the graphs in them; the
/// first folder that holds the graph wins
const FUNCTION_FOLDERS: &[(&str, &str)] = &[
    ("colorValues", "Color"),
    ("Tonemappers", "Color"),
    ("easings", "Easings"),
    ("Math/Parity", "Parity"),
    ("Random", "Random"),
    ("Ranges", "Conversion"),
    ("typeConverters", "Conversion"),
    ("Transforms", "Transformation"),
    ("switch", "Various"),
    ("Various", "Various"),
    ("Cycles", "Various"),
    ("Math", "Maths"),
];

/// The Functions sub-category of a function graph, from its path in the library package
/// (`Functions/Math/Pi` → `Functions/Constant`); `Functions` when nothing matches
pub fn function_category(path: &str) -> String {
    let (folder, name) = path.rsplit_once('/').unwrap_or(("", path));
    let folder = folder.strip_prefix("Functions").map_or(folder, |rest| rest.trim_start_matches('/'));
    let sub = FUNCTION_NAMES
        .iter()
        .find(|(known, _)| *known == name)
        .or_else(|| FUNCTION_PREFIXES.iter().find(|(prefix, _)| name.starts_with(prefix)))
        .or_else(|| FUNCTION_FOLDERS.iter().find(|(known, _)| folder == *known || folder.strip_prefix(known).is_some_and(|rest| rest.starts_with('/'))))
        .map(|(_, sub)| *sub);
    category_path("Functions", sub)
}

/// A readable label from an SBS identifier: `3D_perlin_noise` → "3D Perlin Noise"
pub fn humanize(identifier: &str) -> String {
    const WORDS: &[(&str, &str)] = &[
        ("2d", "2D"),
        ("3d", "3D"),
        ("acescg", "ACEScg"),
        ("ao", "AO"),
        ("bnw", "BnW"),
        ("hdr", "HDR"),
        ("hdri", "HDRI"),
        ("hq", "HQ"),
        ("id", "ID"),
        ("mg", "MG"),
        ("pbr", "PBR"),
        ("rgb", "RGB"),
        ("rgba", "RGBA"),
        ("rgb-a", "RGB-A"),
        ("srgb", "sRGB"),
        ("uv", "UV"),
    ];
    let name = identifier.rsplit('/').next().unwrap_or(identifier);
    name.split(['_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if let Some((_, known)) = WORDS.iter().find(|(from, _)| *from == lower) {
                return (*known).to_string();
            }

            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "node_library.test.rs"]
mod tests;
