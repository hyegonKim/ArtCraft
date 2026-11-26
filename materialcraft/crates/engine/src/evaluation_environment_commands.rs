use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, always, bad, has_graph},
};
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "session.environment",
            "Evaluation environment",
            [],
            None,
            "{timeSeconds?: nonnegative float, uvTile?: [int, int], normalFormat?: int} updates host values; {} reads them; does not modify the package",
            always,
            environment
        ),
        cmd!(
            "graph.context",
            "Graph evaluation context",
            [],
            None,
            "{physicalSize?: [x, y, z], pixelRatio?: 0|1, tiling?: [horizontal, vertical]} updates material-graph metadata used by system variables; {} reads it",
            has_graph,
            graph_context
        ),
    ]
}

fn environment(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "session.environment";
    let mut environment = session.environment;
    let fields = parameters.as_object().ok_or_else(|| bad(command, "expected an object"))?;
    for (name, value) in fields {
        match name.as_str() {
            "timeSeconds" => environment.time_seconds = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?,
            "uvTile" => environment.uv_tile = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?,
            "normalFormat" => environment.normal_format = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?,
            _ => return Err(bad(command, format!("unknown environment field `{name}`"))),
        }
    }

    if !environment.time_seconds.is_finite() || environment.time_seconds < 0.0 {
        return Err(bad(command, "timeSeconds must be finite and nonnegative"));
    }

    if session.environment != environment {
        session.environment = environment;
        session.dirty = true;
        session.cook_epoch = session.cook_epoch.wrapping_add(1);
    }
    Ok(json!(environment))
}

fn graph_context(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.context";
    let graph = session.graph()?;
    // Reusable functions receive these values from their caller and do not serialize base parameters
    if graph.function.is_some() {
        return Err(bad(command, "function graphs use their caller's evaluation context; edit the material graph"));
    }

    let (mut physical_size, mut pixel_ratio, mut tiling) = (graph.physical_size, graph.pixel_ratio, graph.tiling);
    let (mut pixel_ratio_mode, mut tiling_mode) = (graph.pixel_ratio_mode, graph.tiling_mode);
    let fields = parameters.as_object().ok_or_else(|| bad(command, "expected an object"))?;
    for (name, value) in fields {
        match name.as_str() {
            "physicalSize" => physical_size = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?,
            "pixelRatio" => {
                pixel_ratio = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?;
                pixel_ratio_mode = materialcraft_core::ParameterInheritance::Absolute;
            }
            "tiling" => {
                tiling = serde_json::from_value(value.clone()).map_err(|error| bad(command, error.to_string()))?;
                tiling_mode = materialcraft_core::ParameterInheritance::Absolute;
            }
            _ => return Err(bad(command, format!("unknown graph context field `{name}`"))),
        }
    }

    if physical_size.iter().any(|value| !value.is_finite() || *value < 0.0) || !(0..=1).contains(&pixel_ratio) {
        return Err(bad(command, "physicalSize must be finite and nonnegative; pixelRatio must be 0 or 1"));
    }

    if (graph.physical_size, graph.pixel_ratio, graph.tiling, graph.pixel_ratio_mode, graph.tiling_mode)
        != (physical_size, pixel_ratio, tiling, pixel_ratio_mode, tiling_mode)
    {
        session.edit("Graph evaluation context", |session| {
            let graph = session.graph_mut()?;
            graph.physical_size = physical_size;
            graph.pixel_ratio = pixel_ratio;
            graph.tiling = tiling;
            graph.pixel_ratio_mode = pixel_ratio_mode;
            graph.tiling_mode = tiling_mode;
            Ok(())
        })?;
    }
    Ok(json!({"physicalSize": physical_size, "pixelRatio": pixel_ratio, "tiling": tiling}))
}
