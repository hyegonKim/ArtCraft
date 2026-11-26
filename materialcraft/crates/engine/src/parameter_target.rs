use crate::{
    FunctionGraphTarget, Result, Session,
    commands::{bad, node_param, strp},
};
use materialcraft_core::{BaseParameter, BaseParameters, FunctionGraph, Graph, NodeId, Value};
use materialcraft_nodes::ParameterDescriptor;
use serde_json::Value as Json;

#[derive(Clone, Debug)]
pub(crate) enum ParameterTarget {
    Base { node: NodeId, parameter: BaseParameter },
    Specific { node: NodeId, parameter: String },
}

impl ParameterTarget {
    pub(crate) fn resolve(session: &Session, command: &str, parameters: &Json) -> Result<Self> {
        let node = node_param(session, command, parameters, "id")?;
        let parameter = strp(parameters, "param").ok_or_else(|| bad(command, "param is required"))?;
        if parameters.get("base").and_then(Json::as_bool) == Some(true) {
            let parameter = BaseParameter::named(parameter).ok_or_else(|| bad(command, "unknown base parameter"))?;
            Ok(Self::Base { node, parameter })
        } else {
            Ok(Self::Specific { node, parameter: parameter.into() })
        }
    }

    pub(crate) fn node(&self) -> NodeId {
        match self {
            Self::Base { node, .. } | Self::Specific { node, .. } => *node,
        }
    }

    pub(crate) fn identifier(&self) -> &str {
        match self {
            Self::Base { parameter, .. } => parameter.identifier(),
            Self::Specific { parameter, .. } => parameter,
        }
    }

    pub(crate) fn descriptor(&self, session: &Session, command: &str) -> Result<ParameterDescriptor> {
        let node = session.graph()?.node(self.node())?;
        match self {
            Self::Base { parameter, .. } => Ok(ParameterDescriptor::base(*parameter)),
            Self::Specific { parameter, .. } => {
                ParameterDescriptor::for_node(&session.pkg, node, parameter).ok_or_else(|| bad(command, format!("{} has no parameter `{parameter}`", node.def)))
            }
        }
    }

    pub(crate) fn function<'a>(&self, graph: &'a Graph) -> Option<&'a FunctionGraph> {
        let node = graph.node(self.node()).ok()?;
        match self {
            Self::Base { parameter, .. } => node.base.functions.get(parameter),
            Self::Specific { parameter, .. } => node.functions.get(parameter),
        }
    }

    pub(crate) fn current_value(&self, session: &Session, descriptor: &ParameterDescriptor) -> Result<Value> {
        let graph = session.graph()?;
        let node = graph.node(self.node())?;
        if self.function(graph).is_some() {
            let decoder = session.resource_decoder();
            let mut options = materialcraft_eval::Options::new(&decoder);
            options.environment = session.environment;
            options.max_size = session.preview_size;
            options.only = Some(vec![self.node()]);
            let cooked = materialcraft_eval::cook(&session.pkg, session.graph, &options, &session.cache);
            let result = cooked.nodes.get(&self.node()).ok_or_else(|| bad("graph.node.function", "parameter owner was not evaluated"))?;
            if let Some(error) = &result.error {
                return Err(bad("graph.node.function", error.clone()));
            }

            let value = match self {
                Self::Base { parameter, .. } => result.base_values.get(parameter),
                Self::Specific { parameter, .. } => result.parameters.get(parameter),
            };
            return value.cloned().ok_or_else(|| bad("graph.node.function", "parameter function did not produce a value"));
        }

        Ok(match self {
            Self::Base { parameter, .. } => parameter.value(&node.base),
            Self::Specific { parameter, .. } => node
                .bindings
                .get(parameter)
                .and_then(|input| graph.input_value(input))
                .or_else(|| node.params.get(parameter))
                .cloned()
                .unwrap_or_else(|| descriptor.default.clone()),
        })
    }

    pub(crate) fn set_function(&self, graph: &mut Graph, function: FunctionGraph) -> Result<()> {
        let node = graph.node_mut(self.node())?;
        match self {
            Self::Base { parameter, .. } => {
                node.base.functions.insert(*parameter, function);
            }
            Self::Specific { parameter, .. } => {
                node.bindings.remove(parameter);
                node.functions.insert(parameter.clone(), function);
            }
        }

        Ok(())
    }

    pub(crate) fn clear(&self, graph: &mut Graph, reset: bool) -> Result<()> {
        let node = graph.node_mut(self.node())?;
        match self {
            Self::Base { parameter, .. } => {
                node.base.functions.remove(parameter);
                if reset {
                    let defaults = BaseParameters::default();
                    parameter.set_value(&mut node.base, &parameter.value(&defaults)).map_err(|error| bad("graph.node.reset", error))?;
                    parameter.set_inheritance(&mut node.base, parameter.inheritance(&defaults));
                }
            }
            Self::Specific { parameter, .. } => {
                node.functions.remove(parameter);
                node.bindings.remove(parameter);
                if reset {
                    node.params.remove(parameter);
                }
            }
        }

        Ok(())
    }

    pub(crate) fn function_target(&self) -> FunctionGraphTarget {
        match self {
            Self::Base { node, parameter } => FunctionGraphTarget::Base { node: *node, parameter: *parameter },
            Self::Specific { node, parameter } => FunctionGraphTarget::Dynamic { node: *node, parameter: parameter.clone() },
        }
    }
}
