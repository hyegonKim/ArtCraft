use super::ExplorerTarget;
use crate::{MaterialCraftApp, i18n::tr};
use serde_json::json;

/// Activate the target package before dispatching its Explorer action.
pub(super) fn activate(app: &mut MaterialCraftApp, document: u64) -> bool {
    document == app.session.document_id || app.run("package.activate", json!({"document": document})).is_some()
}

/// Dispatch an Explorer action using the selected item and its owning package.
pub(super) fn invoke(app: &mut MaterialCraftApp, ctx: &egui::Context, target: ExplorerTarget, command: &str) {
    if command == "graph.resource.add" {
        let mut parameters = target.parameters();
        parameters["document"] = json!(target.document());
        crate::properties::GraphPropertyDraft::commit(app);
        if app.run(command, parameters).is_some() {
            app.inspected_folder = None;
            app.inspected_graph = None;
        }
        return;
    }

    if !activate(app, target.document()) {
        return;
    }

    let Some(folder) = target.folder(&app.session.pkg) else { return };
    match command {
        "file.importBitmap" | "file.importVector" => {
            let folder = if target.is_resources() { folder.as_str() } else { "Resources" };
            crate::windows::import_resource_into(app, ctx, command == "file.importVector", folder);
        }
        "ui.newGraph" | "ui.newFunctionGraph" => {
            let (kind, name) = if command == "ui.newFunctionGraph" { ("function", "Untitled_Function_Graph_1") } else { ("material", "Untitled_Graph_1") };
            if app.run("file.new", json!({"folder": folder, "kind": kind, "name": name, "template": "empty"})).is_some() {
                app.ui.explorer.selected = Some(ExplorerTarget::Graph { document: target.document(), index: app.session.graph });
            }
        }
        "folder.create" => {
            if let Some(path) = app
                .run(command, json!({"parent": folder, "name": "Untitled_Folder_1"}))
                .and_then(|result| result.get("folder").and_then(|path| path.as_str()).map(str::to_string))
            {
                app.inspected_folder = Some((target.document(), path.clone()));
                app.inspected_graph = None;
                app.ui.explorer.selected = Some(ExplorerTarget::Folder { document: target.document(), path });
            }
        }
        "graph.rename" | "folder.rename" => {
            if let Some(name) = target.name(&app.session.pkg) {
                app.ui.explorer.rename = Some((target, name));
            }
        }
        "explorer.paste" => {
            app.run(command, json!({"folder": folder}));
        }
        "package.close" | "file.reload" => {
            crate::unsaved::guard(app, crate::unsaved::Guarded::Package { document: target.document(), reload: command == "file.reload" })
        }
        "file.save" | "file.saveAs" | "file.saveCopyAs" | "file.exportMaterial" | "file.sendToTextureCraft" => {
            crate::windows::file_action(app, ctx, command);
        }
        crate::menus::EXPLORER_COMPUTE => compute_outputs(app, &target),
        crate::menus::EXPLORER_VIEW_OUTPUTS => {
            if let ExplorerTarget::Graph { index, .. } = target
                && app.run("view.3d.outputs", json!({"graph": index})).is_some()
            {
                app.ui.dock.focus(crate::docking::Panel::View3d);
            }
        }
        crate::menus::EXPLORER_EXPORT_OUTPUTS | crate::menus::EXPLORER_EXPORT_MATERIAL => {
            if let ExplorerTarget::Graph { index, .. } = target
                && (index == app.session.graph || app.run("graph.open", json!({"graph": index})).is_some())
            {
                let tab = if command == crate::menus::EXPLORER_EXPORT_MATERIAL { crate::export::ExportTab::Mcmat } else { crate::export::ExportTab::FromGraph };
                crate::export::open(app, tab);
            }
        }
        crate::menus::EXPLORER_SHOW_IN_FOLDER => show_in_folder(app),
        crate::menus::EXPLORER_OPEN_DEPENDENCIES => open_dependencies(app),
        _ => {
            app.run(command, target.parameters());
        }
    }
}

/// A dependency that is another package file on disk (not a library package or an unsaved one)
pub(super) fn is_package_file(path: &str) -> bool {
    !path.starts_with("sbs://") && !path.starts_with("unsaved:") && !path.is_empty()
}

/// Cook the graph's outputs, or every graph of the package with outputs, at full size
fn compute_outputs(app: &mut MaterialCraftApp, target: &ExplorerTarget) {
    let graphs: Vec<usize> = match target {
        ExplorerTarget::Graph { index, .. } => vec![*index],
        _ => app.session.pkg.graphs.iter().enumerate().filter(|(_, graph)| graph.function.is_none()).map(|(index, _)| index).collect(),
    };
    let (mut milliseconds, mut errors) = (0.0, 0);
    for index in &graphs {
        let Some(result) = app.run("graph.cook", json!({"graph": index, "full": true})) else { return };
        milliseconds += result["ms"].as_f64().unwrap_or(0.0);
        errors += result["nodes"].as_array().map_or(0, |nodes| nodes.iter().filter(|node| !node["error"].is_null()).count());
    }
    let summary = crate::i18n::trf("Computed {count} graph(s) in {ms} ms", &[("count", &graphs.len()), ("ms", &format!("{milliseconds:.0}"))]);
    if errors > 0 {
        app.toast(format!("{summary}; {}", crate::i18n::trf("{count} node(s) failed: see the graph", &[("count", &errors)])), true);
    } else {
        app.toast(summary, false);
    }
}

/// Return the active package path for native file actions.
#[cfg(not(target_arch = "wasm32"))]
fn package_path(app: &MaterialCraftApp) -> Option<String> {
    app.session.open_packages().into_iter().find(|entry| entry.0 == app.session.document_id).and_then(|entry| entry.2.map(str::to_string))
}

/// Reveal the active package file when supported by the platform.
#[cfg(not(target_arch = "wasm32"))]
fn show_in_folder(app: &mut MaterialCraftApp) {
    let Some(path) = package_path(app) else { return };
    if let Err(error) = craft_update::process::reveal(std::path::Path::new(&path)) {
        app.toast(format!("{}: {error}", tr("Could not show the package file")), true);
    }
}

/// Reveal the active package file when supported by the platform.
#[cfg(target_arch = "wasm32")]
fn show_in_folder(_: &mut MaterialCraftApp) {}

/// Open every package file the active package links, next to it in the Explorer
#[cfg(not(target_arch = "wasm32"))]
fn open_dependencies(app: &mut MaterialCraftApp) {
    let document = app.session.document_id;
    let paths: Vec<String> = app.session.pkg.dependencies.keys().filter(|path| is_package_file(path)).cloned().collect();
    for path in paths {
        crate::windows::open_path(app, &path);
    }
    activate(app, document);
}

/// Native dependency-file actions are unavailable in the browser.
#[cfg(target_arch = "wasm32")]
fn open_dependencies(_: &mut MaterialCraftApp) {}

/// Apply a validated graph or folder rename through the engine.
pub(super) fn rename_dialog(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let Some((target, mut name)) = app.ui.explorer.rename.clone() else { return };
    let mut open = true;
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new(tr("Rename"))
        .id(egui::Id::unique("explorer-rename"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let response = ui.text_edit_singleline(&mut name);
            crate::widgets::name(&response, egui::Role::TextInput, "Name");
            if !ctx.memory(|memory| memory.has_focus(response.id)) && !ctx.egui_wants_keyboard_input() {
                response.request_focus();
            }
            accept |= response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                accept |= ui.button(tr("Rename")).clicked();
                cancel |= ui.button(tr("Cancel")).clicked();
            });
        });
    cancel |= ctx.input(|input| input.key_pressed(egui::Key::Escape));
    app.ui.explorer.rename = if open && !cancel { Some((target.clone(), name.clone())) } else { None };
    if accept && activate(app, target.document()) {
        let mut parameters = target.parameters();
        parameters["name"] = json!(name);
        let command = if matches!(target, ExplorerTarget::Folder { .. }) { "folder.rename" } else { "graph.rename" };
        if let Some(result) = app.run(command, parameters) {
            app.ui.explorer.rename = None;
            app.ui.explorer.selected = if let Some(path) = result.get("folder").and_then(|path| path.as_str()) {
                Some(ExplorerTarget::Folder { document: target.document(), path: path.into() })
            } else {
                Some(target)
            };
        }
    }
}
