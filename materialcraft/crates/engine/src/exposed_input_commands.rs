use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph, strp},
    parameter_target::ParameterTarget,
};
use materialcraft_core::{ExposedParameter, FunctionGraph, ParameterEditor, Value, ValueType};
use serde_json::{Value as J, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.exposed.prepare",
            "Prepare graph input",
            [],
            None,
            "{bind: {node, param}, default?} returns input settings without changing the graph",
            has_graph,
            prepare
        ),
        cmd!(
            "graph.exposed.add",
            "Expose parameter",
            [],
            None,
            "{id?, label?, group?, description?, default?, min?, max?, editor?, clamp?, step?, bind?: {node, param}}",
            has_graph,
            add
        ),
    ]
}

fn prepare(session: &mut Session, parameters: &J) -> Result<J> {
    let (input, _) = prepare_input(session, parameters)?;
    Ok(json!(input))
}

fn add(session: &mut Session, parameters: &J) -> Result<J> {
    let (input, target) = prepare_input(session, parameters)?;
    let value_type = ValueType::of(&input.default).ok_or_else(|| bad("graph.exposed.add", "input must have a scalar or vector type"))?;
    let identifier = input.id.clone();
    session.edit("Expose parameter", |session| {
        let graph = session.graph_mut()?;
        graph.exposed.push(input);
        if let Some(target) = target {
            target.set_function(graph, FunctionGraph::input(&identifier, value_type))?;
        }

        Ok(json!({"id": identifier}))
    })
}

fn number(parameters: &J, name: &str) -> Result<Option<f32>> {
    parameters
        .get(name)
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_f64()
                .map(|value| value as f32)
                .filter(|value| value.is_finite())
                .ok_or_else(|| bad("graph.exposed.add", format!("{name} must be a finite number")))
        })
        .transpose()
}

fn prepare_input(session: &Session, parameters: &J) -> Result<(ExposedParameter, Option<ParameterTarget>)> {
    let command = "graph.exposed.add";
    let target = parameters
        .get("bind")
        .map(|binding| {
            ParameterTarget::resolve(
                session,
                command,
                &json!({
                    "id": binding.get("node"), "param": binding.get("param"), "base": binding.get("base").and_then(J::as_bool).unwrap_or(false)
                }),
            )
        })
        .transpose()?;
    if matches!(target, Some(ParameterTarget::Base { .. })) {
        return Err(bad(command, "only Specific Parameters can be exposed as graph inputs"));
    }

    let identifier = match strp(parameters, "id").map(str::trim).filter(|identifier| !identifier.is_empty()) {
        Some(identifier) => {
            if session.graph()?.exposed.iter().any(|input| input.id == identifier) {
                return Err(bad(command, format!("an input is already called `{identifier}`")));
            }

            identifier.to_string()
        }
        None => {
            let target = target.as_ref().ok_or_else(|| bad(command, "id is required"))?;
            let graph = session.graph()?;
            let base = target.identifier();
            (0..=graph.exposed.len())
                .map(|suffix| if suffix == 0 { base.to_string() } else { format!("{base}_{suffix}") })
                .find(|candidate| !graph.exposed.iter().any(|input| input.id == *candidate))
                .ok_or_else(|| bad(command, "cannot allocate an input identifier"))?
        }
    };
    if !identifier.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return Err(bad(command, "identifier must contain only letters, digits and underscores"));
    }

    let descriptor = target.as_ref().map(|target| target.descriptor(session, command)).transpose()?;
    if descriptor.as_ref().is_some_and(|descriptor| !descriptor.supports_function) {
        return Err(bad(command, "this parameter cannot be exposed"));
    }

    let default = if let Some(value) = parameters.get("default").filter(|value| !value.is_null()) {
        let template = descriptor.as_ref().map_or(Value::Float(0.5), |descriptor| descriptor.default.clone());
        serde_json::from_value::<Value>(value.clone()).ok().or_else(|| template.coerce_json(value)).ok_or_else(|| bad(command, "invalid default"))?
    } else {
        match (&target, &descriptor) {
            (Some(target), Some(descriptor)) => target.current_value(session, descriptor)?,
            _ => Value::Float(0.5),
        }
    };
    let value_type = materialcraft_core::ValueType::of(&default).ok_or_else(|| bad(command, "input must have a scalar or vector type"))?;
    if descriptor.as_ref().is_some_and(|descriptor| materialcraft_core::ValueType::of(&descriptor.default) != Some(value_type)) {
        return Err(bad(command, "input type does not match the parameter"));
    }

    if default.as_vec4().is_some_and(|values| values.iter().any(|value| !value.is_finite())) {
        return Err(bad(command, "default must be finite"));
    }

    let default = descriptor.as_ref().and_then(|descriptor| descriptor.default.coerce_json(&json!(default))).unwrap_or(default);
    let source_input = target.as_ref().and_then(|target| {
        let node = session.graph().ok()?.node(target.node()).ok()?;
        let (_, source) = materialcraft_eval::instance_graph(&session.pkg, node)?;
        source.exposed.iter().find(|input| input.id == target.identifier())
    });
    let editor = parameters
        .get("editor")
        .filter(|value| !value.is_null())
        .map(|value| serde_json::from_value::<ParameterEditor>(value.clone()).map_err(|_| bad(command, "unknown parameter editor")))
        .transpose()?
        .or_else(|| source_input.and_then(|input| input.editor));
    if editor.is_some_and(|editor| !ParameterEditor::choices(&default).contains(&editor)) {
        return Err(bad(command, "editor does not support this parameter type"));
    }

    let default = if editor == Some(ParameterEditor::Color) {
        match default {
            Value::Float4(value) => Value::Color(value),
            value => value,
        }
    } else {
        default
    };
    let input = ExposedParameter {
        id: identifier.clone(),
        label: strp(parameters, "label").unwrap_or_else(|| descriptor.as_ref().map_or(identifier.as_str(), |descriptor| descriptor.label.as_str())).into(),
        group: strp(parameters, "group").unwrap_or("").into(),
        description: strp(parameters, "description")
            .map(str::to_string)
            .unwrap_or_else(|| source_input.map_or_else(String::new, |input| input.description.clone())),
        editor: editor.filter(|editor| *editor != ParameterEditor::for_value(&default)),
        clamp: parameters
            .get("clamp")
            .map(|value| value.as_bool().ok_or_else(|| bad(command, "clamp must be a boolean")))
            .transpose()?
            .unwrap_or_else(|| source_input.is_some_and(|input| input.clamp)),
        step: number(parameters, "step")?.or_else(|| source_input.and_then(|input| input.step)),
        default,
        min: number(parameters, "min")?.unwrap_or_else(|| descriptor.as_ref().map_or(0.0, |descriptor| descriptor.min)),
        max: number(parameters, "max")?.unwrap_or_else(|| descriptor.as_ref().map_or(1.0, |descriptor| descriptor.max)),
        options: descriptor.map_or_else(Vec::new, |descriptor| descriptor.options),
    };
    if input.min > input.max {
        return Err(bad(command, "minimum must not exceed maximum"));
    }
    if input.step.is_some_and(|step| step <= 0.0) {
        return Err(bad(command, "step must be greater than zero"));
    }

    Ok((input, target))
}
