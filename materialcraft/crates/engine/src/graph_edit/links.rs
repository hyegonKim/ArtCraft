use super::EditableGraph;
use std::collections::BTreeSet;

pub(crate) fn edit_links<G: EditableGraph>(graph: &mut G, wires: &[(u32, usize)], swapping: bool) -> Result<(), String> {
    if swapping && wires.len() != 2 {
        return Err("select exactly two links to swap".into());
    }
    if wires.is_empty() || wires.len() > 4096 {
        return Err("select between 1 and 4096 links".into());
    }

    let mut seen = BTreeSet::new();
    let mut sources = Vec::with_capacity(wires.len());
    for &(id, slot) in wires {
        if !seen.insert((id, slot)) {
            return Err("a link is listed twice".into());
        }

        let source = graph.nodes().iter().find(|node| G::id(node) == id).and_then(|node| G::links(node).get(slot)).copied().flatten();
        sources.push(source.ok_or_else(|| format!("no link at port {slot} of node {id}"))?);
    }

    // Validate the whole exchange afterward so temporary connections cannot create a false cycle
    for (&(id, slot), source) in wires.iter().zip(sources.into_iter().rev()) {
        if let Some(link) = graph.nodes_mut().iter_mut().find(|node| G::id(node) == id).and_then(|node| G::links_mut(node).get_mut(slot)) {
            *link = swapping.then_some(source);
        }
    }
    Ok(())
}
