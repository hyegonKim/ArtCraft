use std::collections::BTreeMap;

use materialcraft_core::{Value, ValueType};
use materialcraft_nodes::function::{FunctionScope, SystemVariable, node_types};

use crate::{FunctionGraphTarget, FunctionVariable, Result, Session, commands::bad};

fn variable_scope(target: &FunctionGraphTarget, session: &Session) -> FunctionScope {
    match target {
        FunctionGraphTarget::Pattern { .. } => FunctionScope::FxMap,
        FunctionGraphTarget::Parameter { node, parameter }
            if parameter == "function" && session.graph().ok().and_then(|graph| graph.node(*node).ok()).is_some_and(|node| node.def == "pixel_processor") =>
        {
            FunctionScope::PixelProcessor
        }
        _ => FunctionScope::Node,
    }
}

/// Suggestions preserve typed names, even while a graph is being assembled.
pub(crate) fn suggestions(target: &FunctionGraphTarget, session: &Session, id: u32) -> Result<Vec<FunctionVariable>> {
    let command = "graph.function.variables";
    let function = target.current(session)?;
    let node = function.node(id).ok_or_else(|| bad(command, "missing function node"))?;
    if !node.op.starts_with("get_") && node.op != "isdefined" {
        return Err(bad(command, "select a Get or Is Defined node"));
    }

    let types = node_types(function).map_err(|error| bad(command, error))?;
    let expected = (node.op != "isdefined").then(|| types.get(&id).and_then(|code| ValueType::from_code(*code))).flatten();
    let scope = variable_scope(target, session);
    let mut variables: Vec<_> = SystemVariable::ALL
        .into_iter()
        .filter(|variable| variable.available_in(scope) && expected.is_none_or(|expected| expected == variable.value_type()))
        .map(|variable| FunctionVariable { name: variable.name().into(), value_type: variable.value_type(), system: true })
        .collect();

    let document = session.graph()?;
    let mut custom = BTreeMap::new();
    for input in &document.exposed {
        if let Some(value_type) = ValueType::of(&input.default) {
            custom.insert(input.id.clone(), value_type);
        }
    }

    let owner = match target {
        FunctionGraphTarget::Parameter { node, .. } | FunctionGraphTarget::Dynamic { node, .. } | FunctionGraphTarget::Base { node, .. } => Some(*node),
        FunctionGraphTarget::Pattern { owner, .. } => Some(*owner),
        FunctionGraphTarget::Graph => None,
    };
    if let Some(owner) = owner {
        custom.extend(document.node(owner)?.input_values.iter().map(|(name, value_type)| (name.clone(), *value_type)));
    }

    for candidate in &function.nodes {
        if (candidate.op.starts_with("get_") || candidate.op == "set")
            && let Some(Value::Text(name)) = &candidate.value
            && let Some(value_type) = types.get(&candidate.id).and_then(|code| ValueType::from_code(*code))
            && expected.is_none_or(|expected| expected == value_type)
        {
            custom.entry(name.clone()).or_insert(value_type);
        }
    }

    variables.extend(
        custom
            .into_iter()
            .filter(|(name, value_type)| !name.is_empty() && SystemVariable::named(name).is_none() && expected.is_none_or(|expected| expected == *value_type))
            .map(|(name, value_type)| FunctionVariable { name, value_type, system: false }),
    );
    Ok(variables)
}
