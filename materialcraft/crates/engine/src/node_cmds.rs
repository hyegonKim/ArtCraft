//! Node commands: add, move, rename, disable, parameters, base parameters, bindings, wires

use serde_json::{Value as J, json};

use materialcraft_core::{NodeId, OutputFormat, SizeMode, Value, ValueType};

use crate::connection_commands::connection_input;
use crate::graph_grid::snap_position;

use crate::commands::{CommandSpec, bad, boolp, has_graph, has_selection, i64p, ids_param, node_param, point, strp};
use crate::{Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.node.add",
            "Add node",
            [],
            None,
            "{def (see library.list), at? [x, y], snapToGrid? bool, params? {id: value}, connectFrom? {node, port?} wires that output into the new node's first input, label?}",
            has_graph,
            add
        ),
        cmd!("graph.node.move", "Move nodes", [], None, "{ids?, by? [dx, dy], to? [x, y], snapToGrid? bool}", has_selection, move_nodes),
        cmd!("graph.node.rename", "Rename node", [], None, "{id?, label}", has_selection, rename),
        cmd!("graph.node.disable", "Disable selection", [], Some("Shift+D"), "{ids?, disabled? (default: toggle)}", has_selection, disable),
        cmd!(
            "graph.node.param",
            "Set parameter",
            [],
            None,
            "{id?, param, value} (numbers, [x, y], \"#rrggbb\", enum index, typed {t, v})",
            has_graph,
            set_param
        ),
        cmd!(
            "graph.node.base",
            "Set base parameters",
            [],
            None,
            "{id?, param?: outputsize|format|pixelsize|pixelratio|tiling|randomseed, value?, inheritance?: relativeToInput|relativeToParent|absolute}; legacy fields: {sizeMode?, size?: [w, h] (log2), format?: inherit|u8|u16|f16|f32, seed?, tiling?: bool|[bool,bool], pixelRatio?: 0|1, tilingMode?, pixelRatioMode?}",
            has_graph,
            base
        ),
    ]
}

fn add(session: &mut Session, parameters: &J) -> Result<J> {
    if session.graph()?.function.is_some() {
        return Err(bad("graph.node.add", "use function operations in a Function Graph"));
    }

    let definition = strp(parameters, "def").ok_or_else(|| bad("graph.node.add", "`def` is required (see library.list)"))?.to_string();
    let catalog = materialcraft_nodes::find(&definition).ok_or_else(|| bad("graph.node.add", format!("unknown node `{definition}`")))?;

    // Typed parameter values, checked against the catalog
    let mut parameter_values = Vec::new();
    if let Some(values) = parameters.get("params").and_then(J::as_object) {
        for (identifier, value) in values {
            let value = if definition == "graph_instance" && identifier == "graph" {
                Value::Text(value.as_str().ok_or_else(|| bad("graph.node.add", "`graph` is a graph name"))?.into())
            } else if definition == "input_value" && identifier == "default" {
                serde_json::from_value::<Value>(value.clone())
                    .ok()
                    .or_else(|| Value::Float(0.0).coerce_json(value))
                    .filter(|value| ValueType::of(value).is_some())
                    .ok_or_else(|| bad("graph.node.add", "default must be a scalar or vector value"))?
            } else {
                let parameter = catalog.param(identifier).ok_or_else(|| bad("graph.node.add", format!("`{definition}` has no parameter `{identifier}`")))?;
                parameter
                    .default
                    .coerce_json(value)
                    .ok_or_else(|| bad("graph.node.add", format!("`{identifier}` takes a {}", parameter.default.type_name())))?
            };

            crate::graph_validation::value(&value).map_err(|error| bad("graph.node.add", error))?;
            parameter_values.push((identifier.clone(), value));
        }
    }

    let source = parameters
        .get("connectFrom")
        .and_then(|connection| Some((NodeId(connection.get("node")?.as_u64()?), connection.get("port").and_then(J::as_str).unwrap_or("output").to_string())));
    let position = point(parameters, "at").unwrap_or_else(|| {
        let graph = session.graph().ok();
        let source = source.as_ref().and_then(|(node, _)| graph.and_then(|graph| graph.node(*node).ok()));
        source.map_or([0.0, 0.0], |node| [node.pos[0] + 150.0, node.pos[1]])
    });
    let position = snap_position(position, boolp(parameters, "snapToGrid").unwrap_or(false));
    let label = strp(parameters, "label").unwrap_or("").to_string();

    session.edit("Add node", |session| {
        let graph = session.graph_mut()?;
        let id = graph.add_node(&definition, position);
        let node = graph.node_mut(id)?;
        node.label = label;
        if let Some(parameter) = catalog.param("pattern") {
            node.params.insert("pattern".into(), parameter.default.clone());
        }

        if definition == "value_processor" {
            node.input_values.insert("value1".into(), ValueType::Float);
        }
        node.params.extend(parameter_values);

        if let Some((source, port)) = source {
            let input = connection_input(session, source, &port, id)?;
            session.graph_mut()?.connect(source, &port, id, &input)?;
        }
        session.selection = vec![id];
        Ok(json!({"id": id.0}))
    })
}

fn move_nodes(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "graph.node.move", p)?;
    let by = point(p, "by");
    let to = point(p, "to");
    if by.is_none() && to.is_none() {
        return Err(bad("graph.node.move", "`by` or `to` is required"));
    }
    let snap = boolp(p, "snapToGrid").unwrap_or(false);

    s.edit("Move nodes", |s| {
        let g = s.graph_mut()?;
        for n in g.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
            if let Some(t) = to {
                n.pos = snap_position(t, snap);
            } else if let Some(d) = by {
                n.pos = crate::graph_grid::translate(n.pos, d, snap);
            }
        }
        Ok(J::Null)
    })
}

fn rename(s: &mut Session, p: &J) -> Result<J> {
    let id = node_param(s, "graph.node.rename", p, "id")?;
    let label = strp(p, "label").unwrap_or("").trim().to_string();
    s.edit("Rename node", |s| {
        s.graph_mut()?.node_mut(id)?.label = label;
        Ok(J::Null)
    })
}

fn disable(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "graph.node.disable", p)?;
    let set = boolp(p, "disabled");
    s.edit("Disable nodes", |s| {
        let g = s.graph_mut()?;
        for n in g.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
            n.disabled = set.unwrap_or(!n.disabled);
        }
        Ok(J::Null)
    })
}

fn set_param(session: &mut Session, parameters: &J) -> Result<J> {
    let id = node_param(session, "graph.node.param", parameters, "id")?;
    let parameter = strp(parameters, "param").ok_or_else(|| bad("graph.node.param", "`param` is required"))?.to_string();
    let supplied = parameters.get("value").ok_or_else(|| bad("graph.node.param", "`value` is required"))?;
    let node = session.graph()?.node(id)?.clone();
    let value = if node.def == "input_value" && parameter == "default" {
        let current = node.params.get("default").cloned().unwrap_or(Value::Float(0.0));
        serde_json::from_value::<Value>(supplied.clone())
            .ok()
            .or_else(|| current.coerce_json(supplied))
            .filter(|value| materialcraft_core::ValueType::of(value).is_some())
            .ok_or_else(|| bad("graph.node.param", "default must be a scalar or vector value"))?
    } else if node.def == "graph_instance" {
        // Instance parameters come from the source graph's exposed inputs
        if parameter == "graph" {
            Value::Text(supplied.as_str().ok_or_else(|| bad("graph.node.param", "`graph` is a graph name"))?.into())
        } else {
            let source = materialcraft_eval::instance_graph(&session.pkg, &node)
                .map(|(_, g)| g)
                .ok_or_else(|| bad("graph.node.param", "the instanced graph is missing"))?;
            let input = source
                .exposed
                .iter()
                .find(|input| input.id == parameter)
                .ok_or_else(|| bad("graph.node.param", format!("the graph has no input `{parameter}`")))?;
            input.default.coerce_json(supplied).ok_or_else(|| bad("graph.node.param", format!("`{parameter}` takes a {}", input.default.type_name())))?
        }
    } else {
        let definition = materialcraft_nodes::find(&node.def).ok_or_else(|| bad("graph.node.param", format!("unknown node `{}`", node.def)))?;
        let descriptor = definition.param(&parameter).ok_or_else(|| bad("graph.node.param", format!("`{}` has no parameter `{parameter}`", node.def)))?;
        let mut value = descriptor
            .default
            .coerce_json(supplied)
            .ok_or_else(|| bad("graph.node.param", format!("`{parameter}` takes a {}", descriptor.default.type_name())))?;
        if let Value::Enum(i) = &mut value
            && !descriptor.options.is_empty()
        {
            *i = (*i).min(descriptor.options.len() as u32 - 1);
        }

        value
    };

    crate::graph_validation::value(&value).map_err(|error| bad("graph.node.param", error))?;

    session.edit_merged(&format!("param:{}:{parameter}", id.0), "Set parameter", |session| {
        let node = session.graph_mut()?.node_mut(id)?;
        node.functions.remove(&parameter);
        node.bindings.remove(&parameter);
        node.params.insert(parameter.clone(), value);
        Ok(J::Null)
    })
}

fn base(session: &mut Session, parameters: &J) -> Result<J> {
    let id = node_param(session, "graph.node.base", parameters, "id")?;
    if let Some(identifier) = strp(parameters, "param") {
        let parameter = materialcraft_core::BaseParameter::named(identifier).ok_or_else(|| bad("graph.node.base", "unknown base parameter"))?;
        let mut base = session.graph()?.node(id)?.base.clone();
        let inheritance = parameter.inheritance(&base);
        if let Some(value) = parameters.get("value") {
            let value = parameter.value(&base).coerce_json(value).ok_or_else(|| bad("graph.node.base", "incorrect base parameter type"))?;
            parameter.set_value(&mut base, &value).map_err(|error| bad("graph.node.base", error))?;
            parameter.set_inheritance(&mut base, inheritance);
            base.functions.remove(&parameter);
        }
        if let Some(inheritance) = parameters.get("inheritance") {
            let inheritance = serde_json::from_value(inheritance.clone()).map_err(|error| bad("graph.node.base", format!("invalid inheritance: {error}")))?;
            parameter.set_inheritance(&mut base, inheritance);
        }

        return session.edit_merged(&format!("base:{}:{identifier}", id.0), "Set base parameter", |session| {
            session.graph_mut()?.node_mut(id)?.base = base;
            Ok(J::Null)
        });
    }

    let mode = match strp(parameters, "sizeMode") {
        None => None,
        Some("relativeToInput") => Some(SizeMode::RelativeToInput),
        Some("relativeToParent") => Some(SizeMode::RelativeToParent),
        Some("absolute") => Some(SizeMode::Absolute),
        Some(unknown) => return Err(bad("graph.node.base", format!("unknown sizeMode `{unknown}`"))),
    };

    let format = match strp(parameters, "format") {
        None => None,
        Some("inherit") => Some(OutputFormat::Inherit),
        Some("u8") => Some(OutputFormat::U8),
        Some("u16") => Some(OutputFormat::U16),
        Some("f16") => Some(OutputFormat::F16),
        Some("f32") => Some(OutputFormat::F32),
        Some(unknown) => return Err(bad("graph.node.base", format!("unknown format `{unknown}`"))),
    };

    let size = parameters.get("size").and_then(J::as_array).map(|components| {
        [components.first().and_then(J::as_i64).unwrap_or(0).clamp(-12, 13) as i32, components.get(1).and_then(J::as_i64).unwrap_or(0).clamp(-12, 13) as i32]
    });
    let seed = i64p(parameters, "seed").map(|value| value.clamp(0, i32::MAX as i64) as i32);
    let tiling = parameters
        .get("tiling")
        .map(|value| match value.as_bool() {
            Some(value) => Ok([value, value]),
            None => serde_json::from_value::<[bool; 2]>(value.clone()).map_err(|error| bad("graph.node.base", error.to_string())),
        })
        .transpose()?;
    let tiling_mode = parameters
        .get("tilingMode")
        .map(|value| serde_json::from_value(value.clone()).map_err(|error| bad("graph.node.base", error.to_string())))
        .transpose()?;
    let pixel_ratio_mode = parameters
        .get("pixelRatioMode")
        .map(|value| serde_json::from_value(value.clone()).map_err(|error| bad("graph.node.base", error.to_string())))
        .transpose()?;
    let pixel_ratio = parameters
        .get("pixelRatio")
        .map(|value| serde_json::from_value::<i32>(value.clone()).map_err(|error| bad("graph.node.base", error.to_string())))
        .transpose()?;
    if pixel_ratio.is_some_and(|value| !(0..=1).contains(&value)) {
        return Err(bad("graph.node.base", "pixelRatio must be 0 or 1"));
    }

    session.edit_merged(&format!("base:{}", id.0), "Set base parameters", |session| {
        let base = &mut session.graph_mut()?.node_mut(id)?.base;
        if let Some(size_mode) = mode {
            base.size_mode = size_mode;
        }
        if let Some(size) = size {
            base.size = size;
            base.functions.remove(&materialcraft_core::BaseParameter::OutputSize);
        }
        if let Some(format) = format {
            base.format = format;
            base.format_mode = materialcraft_core::ParameterInheritance::Absolute;
            base.functions.remove(&materialcraft_core::BaseParameter::Format);
        }
        if let Some(seed) = seed {
            base.seed = seed;
            base.functions.remove(&materialcraft_core::BaseParameter::RandomSeed);
        }
        if let Some(tiling) = tiling {
            base.tiling = tiling;
            base.functions.remove(&materialcraft_core::BaseParameter::Tiling);
            base.tiling_mode = materialcraft_core::ParameterInheritance::Absolute;
        }
        if let Some(mode) = tiling_mode {
            base.tiling_mode = mode;
        }
        if let Some(value) = pixel_ratio {
            base.pixel_ratio = value;
            base.functions.remove(&materialcraft_core::BaseParameter::PixelRatio);
            base.pixel_ratio_mode = materialcraft_core::ParameterInheritance::Absolute;
        }
        if let Some(mode) = pixel_ratio_mode {
            base.pixel_ratio_mode = mode;
        }

        Ok(J::Null)
    })
}
