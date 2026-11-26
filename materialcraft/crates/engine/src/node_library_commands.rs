//! Library commands: list the categorized library, add an entry to the open graph, and read the
//! library folder again

use crate::commands::{CommandSpec, always, bad, boolp, has_graph, point, strp};
use crate::node_library::{CATEGORIES, LibraryCanvas, LibraryEntry, LibraryItem, NodeLibrary};
use crate::{FunctionGraphTarget, Result, Session, cmd};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const ADD: &str = "library.add";

/// Register catalog queries, insertion and library reload commands.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "library.tree",
            "Library categories",
            [],
            None,
            "{filter?, category? (Top or Top/Sub), canvas? (material|function|pattern)} the library tree and its entries: id, label, category, type, canvas, missing (not addable yet), warning",
            always,
            tree
        ),
        cmd!(
            "library.add",
            "Add library entry",
            [],
            None,
            "{entry (id from library.tree), at? [x, y], snapToGrid?, function?: FunctionGraphTarget, pattern?: owner node id} adds a library entry to the open graph (function entries to a Function canvas)",
            has_graph,
            add
        ),
        cmd!("library.reload", "Reload library", [], None, "{} reads the SBS library folder again", always, reload),
    ]
}

impl Session {
    /// The library's entries, resolved against this session's loaded packages
    pub fn library_entries(&self) -> Vec<LibraryEntry> {
        self.library.entries(&self.pkg.dependencies)
    }
}

/// List entries that match the requested text, category and destination canvas.
fn tree(session: &mut Session, parameters: &Value) -> Result<Value> {
    let filter = strp(parameters, "filter").unwrap_or("").to_lowercase();
    let category = strp(parameters, "category").unwrap_or("");
    let canvas = match strp(parameters, "canvas") {
        None => None,
        Some("material") => Some(LibraryCanvas::Material),
        Some("function") => Some(LibraryCanvas::Function),
        Some("pattern") => Some(LibraryCanvas::Pattern),
        Some(other) => return Err(bad("library.tree", format!("unknown canvas `{other}`: material, function or pattern"))),
    };
    let entries: Vec<_> = session
        .library_entries()
        .into_iter()
        .filter(|entry| category.is_empty() || entry.category == category || entry.category.starts_with(&format!("{category}/")))
        .filter(|entry| canvas.is_none_or(|canvas| entry.canvas == LibraryCanvas::Any || entry.canvas == canvas))
        .filter(|entry| filter.is_empty() || entry.label.to_lowercase().contains(&filter) || entry.id.to_lowercase().contains(&filter))
        .collect();
    let categories: Vec<_> = CATEGORIES.iter().map(|(name, subcategories)| json!({"name": name, "subcategories": subcategories})).collect();
    Ok(json!({"categories": categories, "layouts": session.library.layouts, "entries": entries}))
}

/// Validate an entry and dispatch its insertion through the destination graph command.
fn add(session: &mut Session, parameters: &Value) -> Result<Value> {
    let id = strp(parameters, "entry").ok_or_else(|| bad(ADD, "`entry` is required (an id from library.tree)"))?;
    let entry = session.library_entries().into_iter().find(|entry| entry.id == id).ok_or_else(|| bad(ADD, format!("no library entry `{id}`")))?;
    if let Some(missing) = &entry.missing {
        return Err(bad(ADD, format!("{}: {missing}", entry.label)));
    }

    let at = point(parameters, "at").unwrap_or([0.0, 0.0]);
    let snap = boolp(parameters, "snapToGrid").unwrap_or(false);
    let function_graph = session.graph()?.function.is_some();
    let target = parameters.get("function").cloned().filter(|target| !target.is_null());
    if entry.canvas == LibraryCanvas::Any {
        let destination = crate::AnnotationTarget::resolve(session, parameters)?;
        let mut arguments = destination.parameters();
        arguments["at"] = json!(at);
        let command = match &entry.item {
            LibraryItem::Comment => "graph.comment.add",
            LibraryItem::Frame => {
                arguments["rect"] = json!([at[0], at[1], 400.0, 260.0]);
                "graph.frame.add"
            }
            LibraryItem::Pin => "graph.pin.add",
            LibraryItem::Node { def } if def == "dot" => match crate::SelectionTarget::resolve(session, parameters)? {
                Some(crate::SelectionTarget::Function(target)) => {
                    arguments = json!({"target": target, "edit": {"action": "add", "operation": "passthrough", "at": at, "snap": snap}});
                    "graph.function.edit"
                }
                Some(crate::SelectionTarget::Pattern(owner)) => {
                    arguments = json!({"id": owner.0, "edit": {"action": "add", "kind": "passthrough", "at": at, "snap": snap}});
                    "graph.pattern.edit"
                }
                None => {
                    arguments = json!({"def": "dot", "at": at, "snapToGrid": snap});
                    "graph.node.add"
                }
            },
            _ => return Err(bad(ADD, "unknown graph item")),
        };
        let result = crate::execute(session, command, &arguments)?;
        if matches!(&entry.item, LibraryItem::Node { .. }) {
            session.annotation_selection = None;
        }
        return Ok(json!({"entry": entry.id, "result": result}));
    }
    if entry.canvas == LibraryCanvas::Function {
        let target = match target {
            Some(target) => serde_json::from_value::<FunctionGraphTarget>(target).map_err(|error| bad(ADD, error.to_string()))?,
            None if function_graph => FunctionGraphTarget::Graph,
            None => return Err(bad(ADD, format!("{} goes in a Function canvas: open a Function Graph or pass `function`", entry.label))),
        };
        let target = serde_json::to_value(target).map_err(|error| bad(ADD, error.to_string()))?;
        let result = match &entry.item {
            LibraryItem::Operation { operation } => crate::execute(
                session,
                "graph.function.edit",
                &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": at, "snap": snap}}),
            )?,
            LibraryItem::FunctionInstance { package, graph } => {
                crate::execute(session, "library.function.add", &json!({"package": package, "graph": graph, "function": target, "at": at, "snapToGrid": snap}))?
            }
            _ => return Err(bad(ADD, "unexpected function entry")),
        };
        return Ok(json!({"entry": entry.id, "result": result}));
    }

    if entry.canvas == LibraryCanvas::Pattern {
        return Err(bad(ADD, format!("{} is an FX-Map node: add it with graph.pattern.edit", entry.label)));
    }

    if function_graph || target.is_some() || parameters.get("pattern").is_some() {
        return Err(bad(ADD, format!("{} goes in a material graph, not a Function Graph", entry.label)));
    }

    let result = match &entry.item {
        LibraryItem::Node { def } => crate::execute(session, "graph.node.add", &json!({"def": def, "at": at, "snapToGrid": snap}))?,
        LibraryItem::Comment => crate::execute(session, "graph.comment.add", &json!({"at": at}))?,
        LibraryItem::Frame => crate::execute(session, "graph.frame.add", &json!({"rect": [at[0], at[1], 400.0, 260.0]}))?,
        LibraryItem::Pin => crate::execute(session, "graph.pin.add", &json!({"at": at}))?,
        LibraryItem::Instance { package, graph } => {
            crate::execute(session, "graph.instance.add", &json!({"package": package, "graph": graph, "at": at, "snapToGrid": snap}))?
        }
        _ => return Err(bad(ADD, format!("{} can't be added to a graph", entry.label))),
    };
    Ok(json!({"entry": entry.id, "result": result}))
}

/// Replace library-owned snapshots while retaining explicit user dependencies.
fn replace_library(session: &mut Session, library: std::sync::Arc<NodeLibrary>) {
    let mut changed = BTreeSet::new();
    session.library_sources.retain(|path, owned| session.linked_sources.get(path).is_some_and(|source| std::sync::Arc::ptr_eq(source, owned)));
    for path in session.library_sources.keys() {
        if !library.packages.contains_key(path) {
            session.linked_sources.remove(path);
            changed.insert(path.clone());
        }
    }
    session.library_sources.retain(|path, _| library.packages.contains_key(path));

    for (path, package) in &library.packages {
        if session.linked_sources.contains_key(path) && !session.library_sources.contains_key(path) {
            continue;
        }

        if session.linked_sources.get(path).is_none_or(|current| current.as_ref() != package.as_ref()) {
            session.linked_sources.insert(path.clone(), package.clone());
            changed.insert(path.clone());
        }
        if let Some(source) = session.linked_sources.get(path) {
            session.library_sources.insert(path.clone(), source.clone());
        }
    }
    for warning in &library.warnings {
        session.warn(warning.clone());
    }
    session.library = library;
    session.refresh_links_from(changed);
}

/// Reload the installed library and report the catalog visible to this session.
fn reload(session: &mut Session, _: &Value) -> Result<Value> {
    replace_library(session, NodeLibrary::reload());
    let entries = session.library_entries();
    Ok(json!({"entries": entries.len(), "available": entries.iter().filter(|entry| entry.missing.is_none()).count(), "layouts": session.library.layouts}))
}

#[cfg(test)]
#[path = "node_library_commands.test.rs"]
mod tests;
