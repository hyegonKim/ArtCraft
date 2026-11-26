//! `session.inspect`: everything an agent needs to verify its work without a screenshot

use serde_json::{Value as J, json};

use crate::Session;

pub fn session(session: &Session, requested_graph: Option<usize>) -> J {
    let graph_index = requested_graph.unwrap_or(session.graph);
    let graph = session.pkg.graphs.get(graph_index);
    let cooked = (graph_index == session.graph).then_some(&session.cooked);
    let graph_json = graph.map(|graph| {
        let mut result = json!({
            "index": graph_index,
            "name": graph.name,
            "path": graph.sbs_path(),
            "kind": graph.kind(),
            "function": graph.function,
            "functionWarnings": crate::function_inspection::warnings(graph).into_iter().map(|(target, messages)| json!({"target": target, "messages": messages})).collect::<Vec<_>>(),
            "size": graph.size_px(),
            "format": graph.format,
            "seed": graph.seed,
            "attributes": graph.attributes,
            "exposed": graph.exposed,
            "presets": graph.presets,
            "inputs": graph.inputs,
            "inputImages": graph.input_images,
            "inputImagesLinear": graph.input_images_linear,
            "nodes": graph.nodes.iter().map(|node| {
                let entry = cooked.and_then(|cooked| cooked.nodes.get(&node.id));
                let (inputs, outputs) = materialcraft_eval::ports(&session.pkg, graph, node);
                let node_cooked = entry.map(|entry| json!({
                        "error": entry.error, "ms": entry.ms, "cached": entry.cached,
                        "base": entry.base, "baseValues": entry.base_values, "parameters": entry.parameters,
                        "value": entry.outputs.first().and_then(materialcraft_eval::NodeOutput::as_value),
                        "size": entry.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|image| [image.w, image.h]),
                        "kind": entry.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|image| if image.is_gray() { "gray" } else { "color" }),
                        "mean": entry.outputs.first().and_then(materialcraft_eval::NodeOutput::as_image).map(|image| image.mean()),
                    }));
                json!({
                    "id": node.id.0,
                    "def": node.def,
                    "instance": node.instance,
                    "label": if node.label.is_empty() { materialcraft_nodes::find(&node.def).map_or(node.def.as_str(), |definition| definition.label).to_string() } else { node.label.clone() },
                    "pos": node.pos,
                    "params": node.params,
                    "bindings": node.bindings,
                    "functions": node.functions,
                    "inputValues": node.input_values,
                    "base": node.base,
                    "disabled": node.disabled,
                    "inputs": inputs.iter().map(|port| &port.id).collect::<Vec<_>>(),
                    "outputs": outputs.iter().map(|port| &port.id).collect::<Vec<_>>(),
                    "cooked": node_cooked,
                })
            }).collect::<Vec<_>>(),
            "connections": graph.connections,
            "frames": graph.frames,
            "comments": graph.comments,
            "pins": graph.pins,
        });
        result["physicalSize"] = json!(graph.physical_size);
        result["pixelRatio"] = json!(graph.pixel_ratio);
        result["pixelRatioMode"] = json!(graph.pixel_ratio_mode);
        result["tiling"] = json!(graph.tiling);
        result["tilingMode"] = json!(graph.tiling_mode);
        result
    });

    json!({
        "documents": session.inspect_documents(),
        "package": session.has_package().then(|| json!({"name": session.pkg.name, "path": session.path, "modified": session.modified, "folders": session.pkg.folder_paths(), "folderDescriptions": session.pkg.folder_descriptions, "warnings": session.load_warnings, "resources": session.pkg.resources.iter().map(|entry| json!({"name": entry.name, "folder": entry.folder, "file": entry.file, "missing": entry.bytes.is_empty(), "bytes": entry.bytes.len()})).collect::<Vec<_>>()})),
        "graphs": session.pkg.graphs.iter().enumerate().map(|(index, graph)| json!({"index": index, "name": graph.name, "path": graph.sbs_path(), "nodes": graph.nodes.len(), "kind": graph.kind()})).collect::<Vec<_>>(),
        "openGraph": graph.map(|_| session.graph),
        "graph": graph_json,
        "annotationSelection": session.annotation_selection,
        "annotationTarget": session.annotation_target,
        "selection": session.selection.iter().map(|index| index.0).collect::<Vec<_>>(),
        "view2d": session.view2d,
        "view3d": session.view3d,
        "material3d": {
            "assignments": session.material_preview,
            "channels": session.material_preview.sources(&session.pkg).into_iter().map(|(channel, source)| {
                let image = session.material_image(&source);
                json!({"channel": channel, "source": source, "size": image.map(|image| [image.w, image.h]), "mean": image.map(|image| image.mean())})
            }).collect::<Vec<_>>(),
        },
        "previewSize": session.preview_size,
        "camera": session.camera,
        "environment": session.environment,
        "cookMs": session.cooked.ms,
        "dirty": session.dirty,
        "history": session.history(),
        "historyPreferences": session.history_prefs,
        "canUndo": session.can_undo(),
        "canRedo": session.can_redo(),
        "log": session.log.iter().rev().take(20).collect::<Vec<_>>(),
    })
}
