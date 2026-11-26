use materialcraft_core::FunctionGraph;
use std::collections::{BTreeMap, VecDeque};

pub(super) fn reposition(function: &mut FunctionGraph) -> Result<(), String> {
    if function.nodes.len() > 4096 {
        return Err("function exceeds the 4096-node limit".into());
    }

    let mut pending = BTreeMap::new();
    let mut successors = BTreeMap::<u32, Vec<u32>>::new();
    let mut columns = BTreeMap::<u32, usize>::new();
    let mut ready = VecDeque::new();
    for node in &function.nodes {
        if pending.insert(node.id, node.inputs.iter().flatten().count()).is_some() {
            return Err("duplicate node identifier".into());
        }

        if node.inputs.iter().all(Option::is_none) {
            ready.push_back(node.id);
        }
        for input in node.inputs.iter().flatten() {
            successors.entry(*input).or_default().push(node.id);
        }
    }
    if successors.keys().any(|identifier| !pending.contains_key(identifier)) {
        return Err("connection references a missing node".into());
    }

    let mut visited = 0;
    while let Some(identifier) = ready.pop_front() {
        visited += 1;
        let next_column = columns.get(&identifier).copied().unwrap_or(0) + 1;
        for successor in successors.get(&identifier).into_iter().flatten() {
            let column = columns.entry(*successor).or_default();
            *column = (*column).max(next_column);
            let remaining = pending.get_mut(successor).ok_or("missing successor")?;
            *remaining = remaining.checked_sub(1).ok_or("inconsistent connection count")?;
            if *remaining == 0 {
                ready.push_back(*successor);
            }
        }
    }
    if visited != function.nodes.len() {
        return Err("function contains a connection cycle".into());
    }

    let mut heights = BTreeMap::<usize, f32>::new();
    for node in &mut function.nodes {
        let column = columns.get(&node.id).copied().unwrap_or(0);
        let height = heights.entry(column).or_default();
        node.pos = [64.0 + column as f32 * 280.0, 64.0 + *height];
        *height += (80.0 + node.inputs.len() as f32 * 28.0).max(160.0);
    }

    Ok(())
}
