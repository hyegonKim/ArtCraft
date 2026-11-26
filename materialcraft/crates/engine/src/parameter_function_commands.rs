use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, boolp, has_graph, node_param, strp},
    parameter_target::ParameterTarget,
};
use materialcraft_core::{BaseParameters, FunctionGraph, ValueType};
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.node.function",
            "Set parameter function",
            [],
            None,
            "{id?, param, base?: false, mode?: empty|constant, value?: constant value, function?: {nodes, output}, remove?: false}",
            has_graph,
            set_function
        ),
        cmd!(
            "graph.node.function.edit",
            "Edit parameter function",
            [],
            None,
            "{id?, param, base?: false} resolves the parameter function to open",
            has_graph,
            edit
        ),
        cmd!("graph.node.reset", "Reset parameters", [], None, "{id?, param? (default: all specific parameters), base?: false}", has_graph, reset),
        cmd!("graph.node.bind", "Bind parameter to input", [], None, "{id?, param, base?: false, input: exposed id or null}", has_graph, bind),
        cmd!(
            "graph.node.parameterInfo",
            "Inspect parameter",
            [],
            None,
            "{id?, param, base?: false} returns the parameter's type, driver and inheritance",
            has_graph,
            info
        ),
    ]
}

fn set_function(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.node.function";
    let target = ParameterTarget::resolve(session, command, parameters)?;
    let descriptor = target.descriptor(session, command)?;
    if !descriptor.supports_function {
        return Err(bad(command, "this parameter does not support a function"));
    }

    if boolp(parameters, "remove") == Some(true) {
        return session.edit("Remove parameter function", |session| {
            target.clear(session.graph_mut()?, false)?;
            Ok(Json::Null)
        });
    }

    let mode = strp(parameters, "mode");
    if mode.is_some_and(|mode| !matches!(mode, "empty" | "constant")) {
        return Err(bad(command, "mode must be empty or constant"));
    }

    let function = if let Some(function) = parameters.get("function").filter(|value| !value.is_null()) {
        serde_json::from_value::<FunctionGraph>(function.clone()).map_err(|error| bad(command, error.to_string()))?
    } else if mode == Some("empty") {
        FunctionGraph::default()
    } else if mode.is_none()
        && let Some(function) = target.function(session.graph()?)
    {
        function.clone()
    } else {
        let value = if let Some(value) = parameters.get("value").filter(|value| !value.is_null()) {
            descriptor.default.coerce_json(value).ok_or_else(|| bad(command, "incorrect constant value type"))?
        } else {
            target.current_value(session, &descriptor)?
        };
        FunctionGraph::constant(value)
    };
    crate::graph_validation::function(&function).map_err(|error| bad(command, error))?;
    session.edit("Set parameter function", |session| {
        target.set_function(session.graph_mut()?, function)?;
        Ok(json!({"target": target.function_target()}))
    })
}

fn edit(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.node.function.edit";
    let target = ParameterTarget::resolve(session, command, parameters)?;
    target.descriptor(session, command)?;
    target.function(session.graph()?).ok_or_else(|| bad(command, "parameter has no function"))?;

    Ok(json!({"target": target.function_target()}))
}

fn reset(session: &mut Session, parameters: &Json) -> Result<Json> {
    let node = node_param(session, "graph.node.reset", parameters, "id")?;
    let target = parameters.get("param").map(|_| ParameterTarget::resolve(session, "graph.node.reset", parameters)).transpose()?;
    if let Some(target) = &target {
        target.descriptor(session, "graph.node.reset")?;
    }

    session.edit("Reset parameters", |session| {
        if let Some(target) = target {
            target.clear(session.graph_mut()?, true)?;
        } else {
            let node = session.graph_mut()?.node_mut(node)?;
            if boolp(parameters, "base") == Some(true) {
                node.base = BaseParameters::default();
            } else {
                node.params.retain(|parameter, _| matches!(parameter.as_str(), "graph" | "identifier"));
                node.bindings.clear();
                node.functions.clear();
            }
        }

        Ok(Json::Null)
    })
}

fn bind(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.node.bind";
    let target = ParameterTarget::resolve(session, command, parameters)?;
    let descriptor = target.descriptor(session, command)?;
    let function = match parameters.get("input") {
        Some(Json::Null) => None,
        Some(Json::String(identifier)) => {
            let input = session.graph()?.exposed.iter().find(|input| &input.id == identifier).ok_or_else(|| bad(command, "no such exposed input"))?;
            let expected =
                ValueType::of(&descriptor.default).filter(|_| descriptor.supports_function).ok_or_else(|| bad(command, "this parameter cannot be exposed"))?;
            if ValueType::of(&input.default) != Some(expected) {
                return Err(bad(command, format!("input must be {}", expected.label())));
            }

            Some(FunctionGraph::input(identifier, expected))
        }
        _ => return Err(bad(command, "input must be an exposed identifier or null")),
    };
    session.edit("Bind parameter", |session| {
        match function {
            Some(function) => target.set_function(session.graph_mut()?, function)?,
            None => target.clear(session.graph_mut()?, false)?,
        }

        Ok(Json::Null)
    })
}

fn info(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.node.parameterInfo";
    let target = ParameterTarget::resolve(session, command, parameters)?;
    let descriptor = target.descriptor(session, command)?;
    let graph = session.graph()?;
    let function = target.function(graph);
    let inheritance = match target {
        ParameterTarget::Base { node, parameter } => Some(parameter.inheritance(&graph.node(node)?.base)),
        _ => None,
    };
    let current = target.current_value(session, &descriptor);
    Ok(json!({"target": target.function_target(), "label": descriptor.label, "default": descriptor.default,
        "expectedType": ValueType::of(&descriptor.default), "supportsFunction": descriptor.supports_function,
        "value": current.as_ref().ok(), "error": current.as_ref().err().map(ToString::to_string), "function": function, "input": function.and_then(FunctionGraph::input_identifier),
        "inheritance": inheritance, "warnings": function.map(|function| crate::function_inspection::warnings_for(graph, &target.function_target(), function)).unwrap_or_default()}))
}
