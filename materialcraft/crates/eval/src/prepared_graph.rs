use crate::{Port, ports_with_connections};
use materialcraft_core::{Graph, GraphIndex, NodeId, Package};
use std::collections::BTreeMap;

/// Port definitions resolved once for a cook, alongside borrowed graph lookups
pub(crate) struct PreparedGraph<'a> {
    pub index: GraphIndex<'a>,
    pub order: Vec<NodeId>,
    pub parent_base: crate::resolved_base_parameters::ResolvedBaseParameters,
    ports: BTreeMap<NodeId, (Vec<Port>, Vec<Port>)>,
}

impl<'a> PreparedGraph<'a> {
    pub fn new(package: &Package, graph: &'a Graph, options: &crate::Options<'_>) -> Self {
        let index = GraphIndex::new(graph);
        let needed = options.only.as_deref().map(|roots| index.required_nodes(roots));
        let order: Vec<_> = index.topo_order().into_iter().filter(|id| needed.as_ref().is_none_or(|needed| needed.contains(id))).collect();
        let ports = order
            .iter()
            .filter_map(|id| {
                let node = index.node(*id)?;
                Some((*id, ports_with_connections(package, node, index.incoming(*id).iter().copied())))
            })
            .collect();

        let parent_base = crate::resolved_base_parameters::ResolvedBaseParameters::graph(graph, options);
        Self { index, order, ports, parent_base }
    }

    pub fn input_base(&self, id: NodeId, cooked: &crate::Cooked) -> Option<crate::ResolvedBaseParameters> {
        let node = self.index.node(id)?;
        let definition = materialcraft_nodes::find(&node.def);
        let primary = definition.and_then(|definition| definition.primary_input);
        let mut base = self
            .ports(id)
            .0
            .iter()
            .filter(|port| port.kind != materialcraft_nodes::PortKind::Value && primary.is_none_or(|primary| port.id == primary))
            .find_map(|port| {
                let connection = self.index.input_of(id, &port.id)?;
                cooked.nodes.get(&connection.from).map(|result| result.base)
            })?;

        // Only bypassed generators take their image dimensions from the input
        if !node.disabled && definition.is_some_and(|definition| definition.generator) {
            base.size = self.parent_base.size;
        }

        Some(base)
    }

    pub fn ports(&self, id: NodeId) -> (&[Port], &[Port]) {
        self.ports.get(&id).map_or((&[], &[]), |(inputs, outputs)| (inputs.as_slice(), outputs.as_slice()))
    }
}
