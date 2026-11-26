//! Preventing and breaking loops of components that contain each other's instances (#1084).
//!
//! A main component `M` shows component `T` when an instance of `T` (or an `instanceSwap` property
//! pointing at `T`) lives inside `M`. Sibling variants in a component set are distinct components,
//! so `State=On` may hold an instance of `State=Off` as long as `Off` does not lead back to `On`.
//! Instances placed into a slot of an instance on the canvas belong to that instance's own content
//! rather than to any main component.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_doc::{Document, NodeId, NodeKind, PropDef};

use super::{holds_content, inside_instance, owning_component, rebase_instance, scope, variant_set};

/// Directed edges `owner_component -> target_component` in `doc`.
fn component_edges(doc: &Document) -> BTreeMap<NodeId, BTreeSet<NodeId>> {
    let mut edges: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    for n in doc.nodes() {
        if matches!(n.kind, NodeKind::Component { .. }) {
            edges.entry(n.id).or_default();
        }
        let NodeKind::Instance { component, props } = &n.kind else { continue };
        let Some(owner) = owning_component(doc, n.id) else { continue };
        if doc.node(*component).is_some_and(|m| matches!(m.kind, NodeKind::Component { .. })) {
            edges.entry(owner).or_default().insert(*component);
        }
        for (prop_name, val) in props {
            if is_instance_swap_prop(doc, *component, prop_name)
                && let Some(swapped) = crate::params::id_of(val)
                && doc.node(swapped).is_some_and(|m| matches!(m.kind, NodeKind::Component { .. }))
            {
                edges.entry(owner).or_default().insert(swapped);
            }
        }
    }
    edges
}

fn is_instance_swap_prop(doc: &Document, component: NodeId, name: &str) -> bool {
    let on_comp = match doc.node(component).map(|n| &n.kind) {
        Some(NodeKind::Component { props, .. }) => matches!(props.get(name), Some(PropDef::InstanceSwap { .. })),
        _ => false,
    };
    if on_comp {
        return true;
    }
    let Some(set) = variant_set(doc, component).and_then(|s| doc.node(s)) else {
        return false;
    };
    set.children.iter().any(|c| match doc.node(*c).map(|n| &n.kind) {
        Some(NodeKind::Component { props, .. }) => matches!(props.get(name), Some(PropDef::InstanceSwap { .. })),
        _ => false,
    })
}

fn reaches(edges: &BTreeMap<NodeId, BTreeSet<NodeId>>, start: NodeId, goal: NodeId) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(cur) = stack.pop() {
        if !seen.insert(cur) {
            continue;
        }
        let Some(nexts) = edges.get(&cur) else { continue };
        for &nx in nexts {
            if nx == goal {
                return true;
            }
            stack.push(nx);
        }
    }
    false
}

/// Find a directed cycle `[c0, c1, ..., ck]` (where `ck` links back to `c0`) starting from any of
/// `roots` (in order).
fn find_cycle_from(edges: &BTreeMap<NodeId, BTreeSet<NodeId>>, roots: impl IntoIterator<Item = NodeId>) -> Option<Vec<NodeId>> {
    // 0 = unvisited, 1 = on stack, 2 = done
    let mut state: BTreeMap<NodeId, u8> = BTreeMap::new();
    let mut path: Vec<NodeId> = Vec::new();

    fn dfs(
        u: NodeId,
        edges: &BTreeMap<NodeId, BTreeSet<NodeId>>,
        state: &mut BTreeMap<NodeId, u8>,
        path: &mut Vec<NodeId>,
    ) -> Option<Vec<NodeId>> {
        state.insert(u, 1);
        path.push(u);
        if let Some(nexts) = edges.get(&u) {
            for &v in nexts {
                match state.get(&v).copied().unwrap_or(0) {
                    1 => {
                        let start = path.iter().position(|&x| x == v).unwrap_or(0);
                        return Some(path[start..].to_vec());
                    }
                    0 => {
                        if let Some(cyc) = dfs(v, edges, state, path) {
                            return Some(cyc);
                        }
                    }
                    _ => {}
                }
            }
        }
        path.pop();
        state.insert(u, 2);
        None
    }

    for r in roots {
        if state.get(&r).copied().unwrap_or(0) == 0
            && let Some(cyc) = dfs(r, edges, &mut state, &mut path)
        {
            return Some(cyc);
        }
    }
    None
}

/// Any cycle of components containing each other's instances in `doc`.
pub fn component_cycle(doc: &Document) -> Option<Vec<NodeId>> {
    let edges = component_edges(doc);
    find_cycle_from(&edges, edges.keys().copied())
}

fn component_display_name(doc: &Document, id: NodeId) -> String {
    let Some(n) = doc.node(id) else { return format!("{id}") };
    if let Some(set_n) = variant_set(doc, id).and_then(|s| doc.node(s)) {
        format!("{} ({})", set_n.name, n.name)
    } else {
        n.name.clone()
    }
}

fn format_cycle(doc: &Document, cyc: &[NodeId]) -> String {
    let mut names: Vec<String> = cyc.iter().map(|c| component_display_name(doc, *c)).collect();
    if let Some(first) = cyc.first() {
        names.push(component_display_name(doc, *first));
    }
    names.join(" → ")
}

/// Human-readable description of a component containment cycle in `doc`, if any.
pub fn cycle_text(doc: &Document) -> Option<String> {
    let cyc = component_cycle(doc)?;
    Some(format_cycle(doc, &cyc))
}

fn find_new_cycle(before: &Document, after: &Document) -> Option<Vec<NodeId>> {
    let old_edges = component_edges(before);
    let new_edges = component_edges(after);
    let mut added_owners = Vec::new();
    for (&u, targets) in &new_edges {
        let old_targets = old_edges.get(&u);
        if targets.iter().any(|v| !old_targets.is_some_and(|ot| ot.contains(v))) {
            added_owners.push(u);
        }
    }
    if added_owners.is_empty() {
        return None;
    }
    find_cycle_from(&new_edges, added_owners)
}

/// When changes from `before` to `after` close a new component loop, its formatted names.
pub fn new_cycle<'a>(before: &Document, after: &Document, _changed: impl Iterator<Item = &'a NodeId>) -> Option<String> {
    let cyc = find_new_cycle(before, after)?;
    Some(format_cycle(after, &cyc))
}

/// Refuse edits that introduce a component containment loop.
pub fn check_component_cycles(before: &Document, after: &Document) -> std::result::Result<(), String> {
    if let Some(cyc) = find_new_cycle(before, after)
        && let Some(&first) = cyc.first()
    {
        return Err(contains_itself_error(after, first));
    }
    Ok(())
}

/// Whether placing or swapping an instance of `component` at `target` would make a component
/// contain an instance of itself.
pub fn would_contain_itself(doc: &Document, component: NodeId, target: NodeId) -> bool {
    let Some(owner) = owning_component(doc, target) else { return false };
    if owner == component {
        return true;
    }
    let edges = component_edges(doc);
    reaches(&edges, component, owner)
}

/// Error message when `component` would contain an instance of itself.
pub fn contains_itself_error(doc: &Document, component: NodeId) -> String {
    let name = component_display_name(doc, component);
    format!("\"{name}\" can't contain an instance of itself")
}

/// Detach one instance inside `owner` that causes `owner -> target`, returning a repair note.
fn detach_cyclic_link(doc: &mut Document, owner: NodeId, target: NodeId) -> Option<String> {
    let candidates: Vec<NodeId> = doc
        .subtree(owner)
        .into_iter()
        .filter(|&id| id != owner && owning_component(doc, id) == Some(owner))
        .collect();

    // Prefer detaching a direct (non-nested) instance inside `owner` that leads to `target`.
    let edges = component_edges(doc);
    let mut chosen: Option<NodeId> = None;
    for &id in &candidates {
        if inside_instance(doc, id) {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        let NodeKind::Instance { component, props } = &n.kind else { continue };
        let direct = *component == target || reaches(&edges, *component, target);
        let via_prop = props.iter().any(|(k, v)| {
            is_instance_swap_prop(doc, *component, k)
                && crate::params::id_of(v).is_some_and(|s| s == target || reaches(&edges, s, target))
        });
        if direct || via_prop {
            chosen = Some(id);
            break;
        }
    }
    let inst = chosen.or_else(|| {
        candidates.into_iter().find(|&id| {
            doc.node(id)
                .is_some_and(|n| matches!(&n.kind, NodeKind::Instance { component, .. } if *component == target))
        })
    })?;

    let inst_name = doc.node(inst).map(|n| n.name.clone()).unwrap_or_default();
    let owner_name = component_display_name(doc, owner);

    // Keep main component variable modes on the detached frame, clear instance links on direct layers.
    let main_modes = match doc.node(inst).map(|n| &n.kind) {
        Some(NodeKind::Instance { component, .. }) => doc.node(*component).map(|m| m.var_modes.clone()).unwrap_or_default(),
        _ => BTreeMap::new(),
    };
    let sub = doc.subtree(inst);
    let mut nested = Vec::new();
    for d in sub {
        if d == inst {
            if let Some(n) = doc.node_mut(inst) {
                n.kind = NodeKind::Frame;
                n.source = None;
                n.overrides.clear();
                for (c, m) in &main_modes {
                    n.var_modes.entry(c.clone()).or_insert_with(|| m.clone());
                }
            }
            continue;
        }
        let in_this = doc
            .node(d)
            .and_then(|n| n.parent)
            .and_then(|p| super::owning_instance(doc, p))
            .is_none();
        if !in_this {
            continue;
        }
        if doc.node(d).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) {
            nested.push(d);
        } else if let Some(n) = doc.node_mut(d) {
            n.source = None;
            n.overrides.clear();
        }
    }
    for nn in nested {
        rebase_instance(doc, nn);
    }
    Some(format!(
        "Instance \"{inst_name}\" in \"{owner_name}\" was detached because components can't contain each other"
    ))
}

/// Break every component containment loop in `doc` by detaching one instance per loop, returning
/// repair notes.
pub fn break_component_cycles(doc: &mut Document) -> Vec<String> {
    let mut notes = Vec::new();
    let max_iters = doc.nodes().count().saturating_add(1);
    for _ in 0..max_iters {
        let Some(cyc) = component_cycle(doc) else { break };
        let Some(&u) = cyc.first() else { break };
        let v = cyc.get(1).copied().unwrap_or(u);
        if let Some(note) = detach_cyclic_link(doc, u, v) {
            notes.push(note);
        } else {
            break;
        }
    }
    if !notes.is_empty() {
        let instances: Vec<NodeId> = doc
            .nodes()
            .filter(|n| matches!(n.kind, NodeKind::Instance { .. }))
            .map(|n| n.id)
            .collect();
        for inst in instances {
            if doc.contains(inst) {
                super::sync_instance(doc, inst);
            }
        }
    }
    notes
}

/// Descendants of `component` to copy into `inst` during an instance sync, stopping before any
/// nested instance whose main component is already enclosing `inst` or on the active expansion
/// chain (`None` when `inst` itself would already repeat an enclosing component).
pub(super) fn expansion_layers(doc: &Document, inst: NodeId, component: NodeId) -> Option<Vec<NodeId>> {
    let mut base = BTreeSet::new();
    for a in scope(doc, inst).into_iter().skip(1) {
        if let Some(n) = doc.node(a) {
            match &n.kind {
                NodeKind::Component { .. } => {
                    base.insert(a);
                }
                NodeKind::Instance { component: c, .. } => {
                    base.insert(*c);
                }
                _ => {}
            }
        }
    }
    let mut active = base.clone();
    if !active.insert(component) {
        return None;
    }

    fn collect(
        doc: &Document,
        cur: NodeId,
        root_active: &BTreeSet<NodeId>,
        active: &mut BTreeSet<NodeId>,
        out: &mut Vec<NodeId>,
    ) {
        let Some(n) = doc.node(cur) else { return };
        for &child in &n.children {
            out.push(child);
            let Some(cn) = doc.node(child) else { continue };
            if holds_content(cn) {
                let mut slot_active = root_active.clone();
                collect(doc, child, root_active, &mut slot_active, out);
                continue;
            }
            match &cn.kind {
                NodeKind::Instance { component: sub, .. } => {
                    if active.insert(*sub) {
                        collect(doc, child, root_active, active, out);
                        active.remove(sub);
                    }
                }
                _ => collect(doc, child, root_active, active, out),
            }
        }
    }

    let root_active = active.clone();
    let mut out = Vec::new();
    collect(doc, component, &root_active, &mut active, &mut out);
    Some(out)
}
