//! `threeD.addGraphic` ("Add as Single Graphic") and `threeD.mapGraphic` ("Map Graphic") commands
//! backing the 3D and Materials panel's Graphics section.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::Node;
use vectorcraft_render::effects::{self, MappedGraphic, PlanarFace};

use super::super::appearance::{edit_targets, item_target};
use super::{CommandSpec, EngineError, NodeId, Result, Session, bad, cmd, has_doc, str_param};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "threeD.addGraphic",
            "Add as Single Graphic",
            [],
            None,
            "{ids?: [..] (default: the selected objects), name?: string} add the selected or given objects together as one graphic symbol in the document's Symbols / 3D Graphics library without replacing the objects on the artboard; art inside a graph series' design is refused; one undo step → {name}",
            has_doc,
            add_graphic
        ),
        cmd!(
            "threeD.mapGraphic",
            "Map Graphic",
            [],
            None,
            "{symbol: string, face?: \"front\"|\"back\" (default: \"front\"), x?: number, y?: number, scale?: number, rotation?: number, index?: int (default: the target's applied 3D effect), item?: int|null, ids?|id?: [..], target?: \"object\"|\"contents\"} map a symbol onto the applied Extrude & Bevel, Rotate or Revolve effect of each target object in one undo step → {ids}",
            has_doc,
            map_graphic
        ),
    ]
}

fn add_graphic(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "threeD.addGraphic";
    let ids = edit_targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(C, "select objects to add as a graphic"));
    }
    let d = &s.doc()?.doc;
    let nodes: Vec<Arc<Node>> = ids
        .iter()
        .filter_map(|id| d.node(*id).map(|n| Arc::new(n.clone())))
        .collect();
    if nodes.is_empty() {
        return Err(bad(C, "select objects to add as a graphic"));
    }
    let mut art = if nodes.len() == 1 {
        (*nodes[0]).clone()
    } else {
        Node::group(NodeId(0), nodes)
    };
    crate::cmd::threads::keep_shown_text(d, &mut art);
    let (normalized, size, _) = crate::cmd::brushsym::normalize(art)
        .map_err(|e| bad(C, e.to_string()))?;
    let base = str_param(p, "name")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("Graphic");
    let name = d.unique_symbol_name(base);
    s.edit("Add as Single Graphic", |doc, _| {
        crate::cmd::brushsym::store_symbol(doc, &normalized, &name, size);
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn map_graphic(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "threeD.mapGraphic";
    let symbol = str_param(p, "symbol")
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .ok_or_else(|| bad(C, "pass `symbol`"))?
        .to_string();
    let targets = edit_targets(s, p)?;
    if targets.is_empty() {
        return Err(bad(C, "select an object with an Extrude & Bevel, Rotate or Revolve effect"));
    }
    let d = &s.doc()?.doc;
    if !d.symbols.iter().any(|sym| sym.name == symbol) {
        return Err(bad(C, format!("no symbol named `{symbol}`")));
    }
    let face = match p.get("face") {
        None | Some(Value::Null) => PlanarFace::Front,
        Some(v) => v
            .as_str()
            .and_then(PlanarFace::parse)
            .ok_or_else(|| bad(C, "face must be \"front\" or \"back\""))?,
    };
    let num_field = |key: &str, default: f64| -> Result<f64> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| bad(C, format!("`{key}` must be a number"))),
        }
    };
    let graphic = MappedGraphic {
        symbol,
        face,
        x: num_field("x", 0.0)?,
        y: num_field("y", 0.0)?,
        scale: num_field("scale", 100.0)?,
        rotation: num_field("rotation", 0.0)?,
    };

    let item = item_target(s, p, C)?;
    let explicit_index = match p.get("index") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_u64()
                .ok_or_else(|| bad(C, "`index` must be a non-negative integer"))? as usize,
        ),
    };

    let mut resolved_locations: Vec<(NodeId, Option<usize>, usize, Value)> = Vec::with_capacity(targets.len());
    for (target_id, at) in targets.iter().copied().zip(item.per_target(s, &targets)) {
        let n = d.node(target_id).ok_or(EngineError::NoNode(target_id))?;
        let requested_item = match at {
            Some(at) => at.effects_item(&n.appearance, C)?,
            None => None,
        };
        let (found_item, fx_idx, fx) = find_three_d_slot(n, requested_item, explicit_index, C)?;
        if !effects::takes_graphics(&fx.id) {
            return Err(bad(C, effects::GRAPHICS_NOT_TAKEN));
        }
        let merged = effects::merged_params(&fx.id, &fx.params);
        let updated_merged = effects::with_graphic(&merged, graphic.clone());
        let updated_stored = effects::with_graphic(&fx.params, graphic.clone());
        if fx.visible {
            effects::validate_three_d_at_in(Some(d), n, &fx.id, &updated_merged, (found_item, fx_idx))
                .map_err(|e| bad(C, e))?;
        } else {
            effects::validate_three_d_params(&fx.id, &updated_merged).map_err(|e| bad(C, e))?;
        }
        resolved_locations.push((target_id, found_item, fx_idx, updated_stored));
    }

    s.edit("Map Graphic", |doc, _| {
        for &(id, item_idx, fx_idx, ref new_params) in &resolved_locations {
            let n = doc.node_mut(id).ok_or(EngineError::NoNode(id))?;
            let app = &mut n.appearance;
            let list = match item_idx {
                None => &mut app.effects,
                Some(k) => app
                    .items
                    .get_mut(k)
                    .ok_or_else(|| bad(C, "missing appearance item"))?
                    .effects_mut(),
            };
            let fx = list
                .get_mut(fx_idx)
                .ok_or_else(|| bad(C, "missing 3D effect"))?;
            fx.params = new_params.clone();
        }
        Ok(())
    })?;

    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}

fn find_three_d_slot<'a>(
    n: &'a Node,
    requested_item: Option<usize>,
    explicit_index: Option<usize>,
    cmd: &str,
) -> Result<(Option<usize>, usize, &'a vectorcraft_doc::Effect)> {
    if let Some(item_idx) = requested_item {
        let fx_list = n
            .appearance
            .items
            .get(item_idx)
            .ok_or_else(|| bad(cmd, "no such appearance item"))?
            .effects();
        if let Some(idx) = explicit_index {
            let fx = fx_list
                .get(idx)
                .filter(|e| effects::is_three_d(&e.id))
                .ok_or_else(|| bad(cmd, format!("no 3D effect at index {idx}")))?;
            return Ok((Some(item_idx), idx, fx));
        }
        if let Some((idx, fx)) = fx_list.iter().enumerate().find(|(_, e)| e.visible && effects::is_three_d(&e.id)).or_else(|| {
            fx_list.iter().enumerate().find(|(_, e)| effects::is_three_d(&e.id))
        }) {
            return Ok((Some(item_idx), idx, fx));
        }
    }
    if let Some(idx) = explicit_index {
        let fx = n
            .appearance
            .effects
            .get(idx)
            .filter(|e| effects::is_three_d(&e.id))
            .ok_or_else(|| bad(cmd, format!("no 3D effect at index {idx}")))?;
        return Ok((None, idx, fx));
    }
    if let Some((idx, fx)) = n
        .appearance
        .effects
        .iter()
        .enumerate()
        .find(|(_, e)| e.visible && effects::is_three_d(&e.id))
        .or_else(|| n.appearance.effects.iter().enumerate().find(|(_, e)| effects::is_three_d(&e.id)))
    {
        return Ok((None, idx, fx));
    }
    for (k, item) in n.appearance.items.iter().enumerate() {
        if !item.visible() {
            continue;
        }
        if let Some((idx, fx)) = item.effects().iter().enumerate().find(|(_, e)| e.visible && effects::is_three_d(&e.id)) {
            return Ok((Some(k), idx, fx));
        }
    }
    Err(bad(cmd, "selected object has no Extrude & Bevel, Rotate or Revolve effect"))
}
