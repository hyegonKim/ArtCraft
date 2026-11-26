pub(crate) use super::function_library_drag::FunctionLibraryDrag;
use crate::MaterialCraftApp;
use materialcraft_engine::FunctionEdit;

/// Add `entry` to the open Function canvas, after its last node
pub(crate) fn add(app: &mut MaterialCraftApp, entry: &FunctionLibraryDrag) {
    if let Some(mut editor) = app.function_editor.take() {
        let at = editor.current(app).map_or([0.0; 2], |graph| [graph.nodes.len() as f32 * 140.0, 0.0]);
        insert(app, &mut editor, entry, at);
        app.function_editor = Some(editor);
    }
}

/// The common function nodes on the canvas toolbar, in groups: (operation, what the button shows).
/// Icons where one fits, short text where the node is a value or an operator
pub(crate) const QUICK: &[&[(&str, &str)]] = &[
    &[("const_float1", "F1"), ("const_float2", "F2"), ("const_float3", "F3"), ("const_float4", "F4"), ("const_int1", "I1"), ("const_bool", "B")],
    &[("get_float1", "$1"), ("get_float2", "$2"), ("get_float3", "$3"), ("get_float4", "$4")],
    &[("vector2", "V2"), ("vector3", "V3"), ("vector4", "V4"), ("swizzle1", egui_phosphor::regular::SHUFFLE)],
    &[("add", "+"), ("sub", "\u{2212}"), ("mul", "\u{d7}"), ("div", "\u{f7}"), ("mulscalar", "k\u{d7}"), ("dot", "\u{b7}")],
    &[
        ("lerp", egui_phosphor::regular::SLIDERS_HORIZONTAL),
        ("min", egui_phosphor::regular::ARROW_LINE_DOWN),
        ("max", egui_phosphor::regular::ARROW_LINE_UP),
        ("abs", "|x|"),
    ],
    &[("ifelse", egui_phosphor::regular::GIT_BRANCH), ("sequence", egui_phosphor::regular::LIST_NUMBERS)],
    &[
        ("samplecol", egui_phosphor::regular::EYEDROPPER_SAMPLE),
        ("samplelum", egui_phosphor::regular::EYEDROPPER),
        ("passthrough", egui_phosphor::regular::DOT_OUTLINE),
    ],
];

/// Add `operation` right of the one selected node, wired from it when the new node has an input;
/// in the middle of the view otherwise
pub(crate) fn add_near_selection(app: &mut MaterialCraftApp, operation: &str) {
    let Some(mut editor) = app.function_editor.take() else { return };
    let source = match editor.selection.as_slice() {
        [id] => editor.current(app).and_then(|graph| graph.node(*id)).map(|node| (node.id, node.pos)),
        _ => None,
    };
    let at = source.map_or_else(
        || {
            let center = editor.pan + app.graph.view.size() * 0.5 / editor.zoom;
            [center.x - crate::graph::NODE_W * 0.5, center.y - 40.0]
        },
        |(_, position)| [position[0] + 180.0, position[1]],
    );
    let before = editor.selection.clone();
    editor.edit(app, FunctionEdit::Add { operation: operation.into(), at, snap: app.ui.prefs.snap_to_grid }, false);
    if let Some((from, _)) = source
        && let Some(&id) = editor.selection.first().filter(|_| editor.selection != before)
        && editor.current(app).and_then(|graph| graph.node(id)).is_some_and(|node| !node.inputs.is_empty())
    {
        editor.edit(app, FunctionEdit::Connect { from, to: id, input: 0 }, false);
        editor.selection = vec![id];
    }
    app.function_editor = Some(editor);
}

/// The native operations accepted by the open Function canvas.
pub(crate) fn available_operations(app: &MaterialCraftApp) -> Vec<&'static str> {
    app.function_editor
        .as_ref()
        .and_then(|editor| editor.command_target())
        .map(|target| target.available_operations(&app.session).map(|definition| definition.id).collect())
        .unwrap_or_default()
}

pub(super) fn insert(app: &mut MaterialCraftApp, editor: &mut super::FunctionEditor, entry: &FunctionLibraryDrag, at: [f32; 2]) {
    match entry {
        FunctionLibraryDrag::Operation(operation) => {
            editor.edit(app, FunctionEdit::Add { operation: operation.clone(), at, snap: app.ui.prefs.snap_to_grid }, false)
        }
        FunctionLibraryDrag::Instance { package, graph } => {
            let Some(target) = editor.command_target() else { return };
            if let Some(result) = app.run(
                "library.function.add",
                serde_json::json!({
                    "package": package, "graph": graph, "function": target, "at": at, "snapToGrid": app.ui.prefs.snap_to_grid,
                }),
            ) && let Some(id) = result.get("id").and_then(serde_json::Value::as_u64).and_then(|id| u32::try_from(id).ok())
            {
                editor.selection = vec![id];
                editor.selected_wires.clear();
                editor.inspected_node = Some(id);
            }
        }
    }
}
