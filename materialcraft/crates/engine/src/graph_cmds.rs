//! Graph commands: open/rename/delete graphs, size and seed, frames, comments, pins, exposed inputs, selection, alignment and "create graph from selection"

use serde_json::{Value as J, json};

use materialcraft_core::{MAX_SIZE_LOG2, MIN_SIZE_LOG2, NodeId, OutputFormat, Value};

use crate::commands::{CommandSpec, always, bad, boolp, has_graph, has_selection, i64p, ids_param, strp, u64p};
use crate::{Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("graph.open", "Open graph", [], None, "{graph: index or name}", always, open),
        cmd!("graph.delete", "Delete graph", [], None, "{graph?}", has_graph, delete),
        cmd!("graph.duplicate", "Duplicate graph", [], None, "{graph?}", has_graph, duplicate),
        cmd!("graph.size", "Graph size", [], None, "{graph?, document?, width, height} in pixels (powers of two, 16..8192) or {log2: [w, h]}", always, size),
        cmd!("graph.seed", "Random seed", [], None, "{seed, graph?, document?}", always, |s, p| {
            let seed = i64p(p, "seed").ok_or_else(|| bad("graph.seed", "`seed` is required"))?.clamp(0, i32::MAX as i64) as i32;
            let target = crate::GraphTarget::resolve(s, p)?;
            target.edit_merged(s, &format!("graph.seed:{}:{}", target.document, target.graph), "Random seed", |package| {
                package.graph_mut(target.graph)?.seed = seed;
                Ok(J::Null)
            })
        }),
        cmd!("graph.format", "Output format", [], None, "{format: u8|u16|f16|f32, graph?, document?}", always, format),
        cmd!(
            "graph.cook",
            "Compute graph",
            [],
            None,
            "{full?: bool, graph?: index (another graph of the package cooks at full size into the cache)} cooks now; returns per-node timings and errors",
            has_graph,
            cook
        ),
        cmd!("graph.exposed.set", "Set input value", [], None, "{id, value}", has_graph, exposed_set),
        cmd!("graph.exposed.delete", "Delete input", [], None, "{id}", has_graph, exposed_delete),
        cmd!(
            "graph.preset.save",
            "Save preset",
            [],
            None,
            "{name} stores the current input values as a preset (replacing one of that name)",
            has_graph,
            preset_save
        ),
        cmd!("graph.preset.apply", "Apply preset", [], None, "{name}", has_graph, preset_apply),
        cmd!("graph.preset.delete", "Delete preset", [], None, "{name}", has_graph, preset_delete),
        cmd!(
            "graph.preset.exportJson",
            "Export presets as JSON",
            [],
            None,
            "{name?} serializes one or all presets of the open graph as portable JSON",
            has_graph,
            preset_export_json
        ),
        cmd!(
            "graph.preset.importJson",
            "Import presets from JSON",
            [],
            None,
            "{json?, presets?, apply?: bool} imports preset definitions into the open graph",
            has_graph,
            preset_import_json
        ),
        cmd!("graph.select", "Select nodes", [], None, "{ids, add?: bool}", has_graph, select),
        cmd!("graph.selectUpstream", "Select upstream nodes", [], None, "{ids?}", has_selection, |s, p| select_stream(s, p, true)),
        cmd!("graph.selectDownstream", "Select downstream nodes", [], None, "{ids?}", has_selection, |s, p| select_stream(s, p, false)),
        cmd!(
            "graph.createFromSelection",
            "Create graph from selection",
            [],
            None,
            "{name?, ids?} moves the nodes into a new graph and replaces them with an instance",
            has_selection,
            from_selection
        ),
    ]
}

fn graph_index(s: &Session, p: &J) -> Result<usize> {
    match p.get("graph") {
        None | Some(J::Null) => Ok(s.graph),
        Some(J::String(name)) => s.pkg.graph_by_name(name).map(|(i, _)| i).ok_or_else(|| bad("graph", format!("no graph `{name}`"))),
        Some(v) => {
            let i = v.as_u64().ok_or_else(|| bad("graph", "`graph` is an index or a name"))? as usize;
            s.pkg.graph(i)?;
            Ok(i)
        }
    }
}

fn open(s: &mut Session, p: &J) -> Result<J> {
    let i = graph_index(s, p)?;
    s.graph = i;
    s.open_material_preview();
    s.selection.clear();
    s.annotation_selection = None;
    s.view2d.node = None;
    s.dirty = true;
    Ok(json!({"graph": i, "name": s.pkg.graphs.get(i).map(|g| g.name.clone())}))
}

fn delete(s: &mut Session, p: &J) -> Result<J> {
    let i = graph_index(s, p)?;
    s.edit("Delete graph", |s| {
        s.pkg.folders = s.pkg.folder_paths().into_iter().collect();
        s.pkg.graphs.remove(i);
        if i < s.graph {
            s.graph -= 1;
        }
        s.material_preview.remove_graph(i);
        s.clear_preview_results();
        s.clipboard = None;
        s.embedded_clipboard = None;
        s.graph = s.graph.min(s.pkg.graphs.len().saturating_sub(1));
        s.selection.clear();
        s.annotation_selection = None;
        s.view2d.node = None;
        Ok(json!({"graphs": s.pkg.graphs.len()}))
    })
}

fn duplicate(s: &mut Session, p: &J) -> Result<J> {
    let i = graph_index(s, p)?;
    s.edit("Duplicate graph", |s| {
        let mut g = s.pkg.graph(i)?.clone();
        g.name = crate::folder_path::unique(&s.pkg, &g.folder, &g.name);
        s.pkg.graphs.push(g);
        s.graph = s.pkg.graphs.len() - 1;
        s.open_material_preview();
        s.selection.clear();
        s.annotation_selection = None;
        Ok(json!({"graph": s.graph}))
    })
}

fn size(s: &mut Session, p: &J) -> Result<J> {
    let target = crate::GraphTarget::resolve(s, p)?;
    let log = |px: u64| 63 - px.clamp(1, 1 << 20).leading_zeros();
    let (w, h) = if let Some(a) = p.get("log2").and_then(J::as_array) {
        (a.first().and_then(J::as_u64).unwrap_or(10) as u32, a.get(1).and_then(J::as_u64).unwrap_or(10) as u32)
    } else {
        let w = u64p(p, "width").ok_or_else(|| bad("graph.size", "`width` is required"))?;
        (log(w), log(u64p(p, "height").unwrap_or(w)))
    };

    let (w, h) = (w.clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2), h.clamp(MIN_SIZE_LOG2, MAX_SIZE_LOG2));
    target.edit(s, "Graph size", |package| {
        package.graph_mut(target.graph)?.size = [w, h];
        Ok(json!({"width": 1u32 << w, "height": 1u32 << h}))
    })
}

fn format(s: &mut Session, p: &J) -> Result<J> {
    let target = crate::GraphTarget::resolve(s, p)?;
    let f = match strp(p, "format") {
        Some("u8" | "8") => OutputFormat::U8,
        Some("u16" | "16") => OutputFormat::U16,
        Some("f16") => OutputFormat::F16,
        Some("f32" | "32") => OutputFormat::F32,
        _ => return Err(bad("graph.format", "`format` is u8, u16, f16 or f32")),
    };

    target.edit(s, "Output format", |package| {
        package.graph_mut(target.graph)?.format = f;
        Ok(J::Null)
    })
}

fn cook(s: &mut Session, p: &J) -> Result<J> {
    let index = match p.get("graph").filter(|graph| !graph.is_null()) {
        None => s.graph,
        Some(graph) => {
            let index = graph.as_u64().and_then(|index| usize::try_from(index).ok()).ok_or_else(|| bad("graph.cook", "`graph` is a graph index"))?;
            let graph = s.pkg.graph(index)?;
            if graph.function.is_some() {
                return Err(bad("graph.cook", "a Function Graph has no outputs to compute"));
            }
            index
        }
    };
    let cooked = if index != s.graph || boolp(p, "full") == Some(true) { s.cook_full(index) } else { s.cook().clone() };
    let g = s.pkg.graph(index)?;
    Ok(json!({
        "ms": cooked.ms,
        "nodes": cooked.nodes.iter().map(|(id, r)| {
            let n = g.node(*id).ok();
            json!({"id": id.0, "def": n.map(|n| n.def.clone()), "ms": r.ms, "cached": r.cached, "error": r.error,
                   "size": r.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|i| [i.w, i.h])})
        }).collect::<Vec<_>>(),
    }))
}

/// Graph-space bounding rect of nodes `ids` (node cards are about 96×110)
pub fn nodes_rect(s: &Session, ids: &[NodeId]) -> Option<[f32; 4]> {
    let g = s.graph().ok()?;
    let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for n in g.nodes.iter().filter(|n| ids.contains(&n.id)) {
        r = [r[0].min(n.pos[0]), r[1].min(n.pos[1]), r[2].max(n.pos[0] + 96.0), r[3].max(n.pos[1] + 110.0)];
    }
    (r[0] < f32::MAX).then(|| [r[0], r[1], r[2] - r[0], r[3] - r[1]])
}

fn exposed_set(s: &mut Session, p: &J) -> Result<J> {
    let id = strp(p, "id").ok_or_else(|| bad("graph.exposed.set", "`id` is required"))?.to_string();
    let v = p.get("value").ok_or_else(|| bad("graph.exposed.set", "`value` is required"))?;
    let cur = s.graph()?.exposed.iter().find(|e| e.id == id).map(|e| e.default.clone()).ok_or_else(|| bad("graph.exposed.set", format!("no input `{id}`")))?;
    let val = cur.coerce_json(v).ok_or_else(|| bad("graph.exposed.set", format!("`{id}` takes a {}", cur.type_name())))?;
    s.edit_merged(&format!("exposed:{id}"), "Set input", |s| {
        s.graph_mut()?.inputs.insert(id.clone(), val);
        Ok(J::Null)
    })
}

fn exposed_delete(session: &mut Session, parameters: &J) -> Result<J> {
    let identifier = strp(parameters, "id").ok_or_else(|| bad("graph.exposed.delete", "`id` is required"))?.to_string();
    session.edit("Delete input", |session| {
        let graph = session.graph_mut()?;
        graph.exposed.retain(|input| input.id != identifier);
        graph.inputs.remove(&identifier);
        for node in &mut graph.nodes {
            node.bindings.retain(|_, input| *input != identifier);

            // Detach direct input links without discarding user-authored function logic
            node.functions.retain(|_, function| function.input_identifier() != Some(identifier.as_str()));
            node.base.functions.retain(|_, function| function.input_identifier() != Some(identifier.as_str()));
        }

        Ok(J::Null)
    })
}

fn preset_name(p: &J, cmd: &str) -> Result<String> {
    strp(p, "name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).ok_or_else(|| bad(cmd, "`name` is required"))
}

fn preset_save(s: &mut Session, p: &J) -> Result<J> {
    let name = preset_name(p, "graph.preset.save")?;
    s.edit("Save preset", |s| {
        let g = s.graph_mut()?;
        let values = g.exposed.iter().filter_map(|e| g.input_value(&e.id).map(|v| (e.id.clone(), v.clone()))).collect();
        g.presets.retain(|x| x.name != name);
        g.presets.push(materialcraft_core::Preset { name: name.clone(), values });
        Ok(json!({"presets": g.presets.len()}))
    })
}

fn preset_apply(s: &mut Session, p: &J) -> Result<J> {
    let name = preset_name(p, "graph.preset.apply")?;
    s.edit("Apply preset", |s| {
        let g = s.graph_mut()?;
        let preset = g.presets.iter().find(|x| x.name == name).cloned().ok_or_else(|| bad("graph.preset.apply", format!("no preset `{name}`")))?;
        for (k, v) in preset.values {
            // Only inputs that still exist, with their current type
            if g.exposed.iter().any(|e| e.id == k && e.default.type_name() == v.type_name()) {
                g.inputs.insert(k, v);
            }
        }
        Ok(J::Null)
    })
}

fn preset_delete(s: &mut Session, p: &J) -> Result<J> {
    let name = preset_name(p, "graph.preset.delete")?;
    s.edit("Delete preset", |s| {
        s.graph_mut()?.presets.retain(|x| x.name != name);
        Ok(J::Null)
    })
}

fn preset_export_json(s: &mut Session, p: &J) -> Result<J> {
    let g = s.graph()?;
    let selected: Vec<materialcraft_core::Preset> = if let Some(name) = strp(p, "name").map(str::trim).filter(|n| !n.is_empty()) {
        let preset = g.presets.iter().find(|x| x.name == name).cloned().ok_or_else(|| bad("graph.preset.exportJson", format!("no preset `{name}`")))?;
        vec![preset]
    } else {
        g.presets.clone()
    };
    let payload = json!({
        "format": "materialcraft-presets-v1",
        "graph": g.name,
        "presets": selected,
    });
    let text = serde_json::to_string_pretty(&payload).map_err(|e| bad("graph.preset.exportJson", e.to_string()))?;
    Ok(json!({
        "format": "materialcraft-presets-v1",
        "graph": g.name,
        "presets": selected,
        "json": text,
    }))
}

fn parse_presets_payload(p: &J) -> Result<Vec<materialcraft_core::Preset>> {
    let raw_list = if let Some(text) = strp(p, "json") {
        let parsed: J = serde_json::from_str(text).map_err(|e| bad("graph.preset.importJson", format!("invalid preset JSON: {e}")))?;
        if parsed.is_array() {
            parsed
        } else {
            parsed.get("presets").cloned().ok_or_else(|| bad("graph.preset.importJson", "JSON object must contain a `presets` array"))?
        }
    } else if let Some(arr) = p.get("presets") {
        arr.clone()
    } else {
        return Err(bad("graph.preset.importJson", "`json` or `presets` is required"));
    };

    let items: Vec<materialcraft_core::Preset> =
        serde_json::from_value(raw_list).map_err(|e| bad("graph.preset.importJson", format!("invalid preset entry: {e}")))?;
    let cleaned: Vec<materialcraft_core::Preset> = items
        .into_iter()
        .filter_map(|mut item| {
            let trimmed = item.name.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                item.name = trimmed;
                Some(item)
            }
        })
        .collect();
    if cleaned.is_empty() {
        return Err(bad("graph.preset.importJson", "no valid presets found to import"));
    }
    Ok(cleaned)
}

fn preset_import_json(s: &mut Session, p: &J) -> Result<J> {
    let incoming = parse_presets_payload(p)?;
    let apply_first = boolp(p, "apply") == Some(true);
    let imported_count = incoming.len();
    let first_name = incoming.first().map(|item| item.name.clone());

    s.edit("Import presets", |s| {
        let g = s.graph_mut()?;
        for mut preset in incoming {
            let mut coerced = std::collections::BTreeMap::new();
            for (k, v) in preset.values {
                if let Some(exp) = g.exposed.iter().find(|e| e.id == k) {
                    if exp.default.type_name() == v.type_name() {
                        coerced.insert(k, v);
                    } else if let Some(conv) = exp.default.coerce_json(&json!(v)) {
                        coerced.insert(k, conv);
                    }
                } else {
                    coerced.insert(k, v);
                }
            }
            preset.values = coerced;
            g.presets.retain(|existing| existing.name != preset.name);
            g.presets.push(preset);
        }

        if apply_first
            && let Some(target_name) = first_name
            && let Some(preset) = g.presets.iter().find(|x| x.name == target_name).cloned()
        {
            for (k, v) in preset.values {
                if g.exposed.iter().any(|e| e.id == k && e.default.type_name() == v.type_name()) {
                    g.inputs.insert(k, v);
                }
            }
        }

        Ok(json!({
            "imported": imported_count,
            "presets": g.presets.len(),
        }))
    })
}

fn select(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "graph.select", p)?;
    if boolp(p, "add") == Some(true) {
        for id in ids {
            if !s.selection.contains(&id) {
                s.selection.push(id);
            }
        }
    } else {
        s.selection = ids;
    }
    Ok(json!({"selection": s.selection.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn select_stream(s: &mut Session, p: &J, up: bool) -> Result<J> {
    let ids = ids_param(s, "graph.select", p)?;
    let g = s.graph()?;
    let mut sel = ids.clone();
    for id in ids {
        for n in if up { g.upstream(id) } else { g.downstream(id) } {
            if !sel.contains(&n) {
                sel.push(n);
            }
        }
    }
    s.selection = sel;
    Ok(json!({"selected": s.selection.len()}))
}

fn from_selection(s: &mut Session, p: &J) -> Result<J> {
    let ids = ids_param(s, "graph.createFromSelection", p)?;
    let base = crate::folder_path::name(strp(p, "name").unwrap_or("Sub_graph"))?;
    s.edit("Create graph from selection", |s| {
        let name = crate::folder_path::unique(&s.pkg, "", base);
        let src = s.graph()?.clone();
        let mut sub = materialcraft_core::Graph::new(&name);
        sub.size = src.size;
        sub.seed = src.seed;
        sub.next_id = src.next_id;
        sub.nodes = src.nodes.iter().filter(|n| ids.contains(&n.id)).cloned().collect();
        sub.connections = src.connections.iter().filter(|c| ids.contains(&c.from) && ids.contains(&c.to)).cloned().collect();
        let incoming: Vec<_> = src.connections.iter().filter(|c| ids.contains(&c.to) && !ids.contains(&c.from)).cloned().collect();
        let outgoing: Vec<_> = src.connections.iter().filter(|c| ids.contains(&c.from) && !ids.contains(&c.to)).cloned().collect();
        let rect = nodes_rect(s, &ids).unwrap_or([0.0; 4]);
        // One graph input per incoming wire, one output per distinct outgoing port
        let mut inputs = Vec::new();
        for (k, c) in incoming.iter().enumerate() {
            let ident = format!("input_{}", k + 1);
            let id = sub.add_node("input_color", [rect[0] - 160.0, rect[1] + k as f32 * 120.0]);
            sub.node_mut(id)?.params.insert("identifier".into(), Value::Text(ident.clone()));
            sub.connect(id, "output", c.to, &c.to_port)?;
            inputs.push((c.from, c.from_port.clone(), ident));
        }
        let mut outputs: Vec<(NodeId, String, String)> = Vec::new();
        for c in &outgoing {
            if outputs.iter().any(|(n, p, _)| *n == c.from && *p == c.from_port) {
                continue;
            }
            let ident = format!("output_{}", outputs.len() + 1);
            let id = sub.add_node("output", [rect[0] + rect[2] + 80.0, rect[1] + outputs.len() as f32 * 120.0]);
            let usage = materialcraft_core::ComponentUsage::ANY_INDEX;
            sub.node_mut(id)?.params.extend([("identifier".to_string(), Value::Text(ident.clone())), ("usage".to_string(), Value::Enum(usage))]);
            sub.connect(c.from, &c.from_port, id, "input")?;
            outputs.push((c.from, c.from_port.clone(), ident));
        }
        if !sub.nodes.iter().any(|node| node.def == "output") {
            return Err(bad("graph.createFromSelection", "the selection needs an output node or an outgoing connection before it can become an instance"));
        }
        s.pkg.graphs.push(sub);
        let g = s.graph_mut()?;
        g.remove_nodes(&ids);
        let inst = g.add_node("graph_instance", [rect[0], rect[1]]);
        g.node_mut(inst)?.params.insert("graph".into(), Value::Text(name.clone()));
        for (from, port, ident) in inputs {
            g.connect(from, &port, inst, &ident)?;
        }
        for c in outgoing {
            if let Some((_, _, ident)) = outputs.iter().find(|(n, p, _)| *n == c.from && *p == c.from_port) {
                g.connect(inst, ident, c.to, &c.to_port)?;
            }
        }
        s.selection = vec![inst];
        Ok(json!({"graph": name, "instance": inst.0}))
    })
}

#[cfg(test)]
#[path = "graph_cmds.test.rs"]
mod tests;
