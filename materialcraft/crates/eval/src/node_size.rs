use materialcraft_core::{Graph, Node};
use materialcraft_nodes::NodeDefinition;

/// Literal output size; cooking also evaluates any attached parameter function
pub fn node_size(graph: &Graph, definition: Option<&NodeDefinition>, node: &Node, input: Option<(u32, u32)>, maximum: Option<u32>) -> (u32, u32) {
    let parent = crate::ResolvedBaseParameters { size: graph.size_px(), ..Default::default() };
    let input = input
        .filter(|_| !definition.is_some_and(|definition| definition.generator))
        .map(|(width, height)| crate::ResolvedBaseParameters { size: [width, height], ..parent });
    let resolved = crate::ResolvedBaseParameters::resolve(&node.base, input, parent);

    limit(resolved.size, maximum)
}

pub(crate) fn limit([width, height]: [u32; 2], maximum: Option<u32>) -> (u32, u32) {
    match maximum {
        Some(maximum) if width.max(height) > maximum => {
            let scale = f64::from(maximum.max(1)) / f64::from(width.max(height));
            (((f64::from(width) * scale).round() as u32).max(1), ((f64::from(height) * scale).round() as u32).max(1))
        }
        _ => (width, height),
    }
}
