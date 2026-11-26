//! The package tree, its graphs, inputs, outputs and resources

use egui::Ui;

mod actions;
mod context_menu;
mod explorer_menu_state;
pub(crate) mod explorer_row;
mod explorer_state;
mod explorer_target;
mod explorer_tree;
mod tree_row;
use egui_phosphor::regular as icon;
pub use explorer_state::ExplorerState;
pub use explorer_target::ExplorerTarget;
use serde_json::json;

use crate::MaterialCraftApp;
use crate::i18n::tr;

/// Draw open packages and dispatch selection, resource and context-menu actions.
pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if crate::widgets::icon_button(ui, icon::FILE_PLUS, tr("New graph..."), false, 22.0).clicked() {
            crate::menus::invoke(app, ui.ctx(), "ui.newGraph");
        }
        if crate::widgets::icon_button(ui, icon::FOLDER_OPEN, tr("Open package..."), false, 22.0).clicked() {
            crate::menus::invoke(app, ui.ctx(), "file.open");
        }
        ui.add_enabled_ui(app.session.has_package(), |ui| {
            if crate::widgets::icon_button(ui, icon::FLOPPY_DISK, tr("Save package"), false, 22.0).clicked() {
                crate::menus::invoke(app, ui.ctx(), "file.save");
            }
            if crate::widgets::icon_button(ui, icon::EXPORT, tr("Export outputs..."), false, 22.0).clicked() {
                crate::export::open(app, crate::export::ExportTab::FromGraph);
            }
            if crate::widgets::icon_button(ui, icon::IMAGE, tr("Import bitmap..."), false, 22.0).clicked() {
                crate::windows::import_resource(app, ui.ctx(), false);
            }
        });
    });
    ui.add_space(2.0);
    let context = ui.ctx().clone();
    let mut tree = explorer_tree::ExplorerTree {
        session: &app.session,
        state: &mut app.ui.explorer,
        action: None,
        opened_node: None,
        assignment: None,
        movement: None,
        inspected_graph: None,
        inspected_folder: None,
    };
    let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (document, package, path, modified) in tree.session.open_packages() {
            ui.push_id(document, |ui| tree.package(ui, document, package, path, modified));
        }
    });
    tree.state.hovered = ui.rect_contains_pointer(scroll.inner_rect);
    let action = tree.action;
    let opened_node = tree.opened_node;
    let assignment = tree.assignment;
    let movement = tree.movement;
    let inspected_folder = tree.inspected_folder;
    if let Some(target) = tree.inspected_graph {
        crate::properties::GraphPropertyDraft::commit(app);
        app.inspected_graph = Some(target);
        app.inspected_folder = None;
        app.ui.dock.focus(crate::docking::Panel::Properties);
    }
    if let Some(target) = inspected_folder {
        crate::properties::GraphPropertyDraft::commit(app);
        app.inspected_folder = Some(target);
        app.inspected_graph = None;
        app.ui.dock.focus(crate::docking::Panel::Properties);
    }
    if let Some((document, index, node)) = opened_node {
        if !actions::activate(app, document) {
            return;
        }

        if index != app.session.graph && app.run("graph.open", json!({"graph": index})).is_none() {
            return;
        }

        app.pattern_editor = None;
        crate::properties::GraphPropertyDraft::commit(app);
        app.inspected_graph = None;
        app.inspected_folder = None;
        app.function_editor = None;
        app.run("graph.annotation.select", json!({}));
        app.run("graph.select", json!({"ids": [node.0]}));
        app.graph.inspected_node = Some(node);
        app.ui.dock.focus(crate::docking::Panel::Properties);
        app.run("view.2d", json!({"node": node.0}));
        app.ui.dock.focus(crate::docking::Panel::View2d);
    }
    if let Some(parameters) = assignment {
        app.run("view.3d.assign", parameters);
    }
    if let Some((target, parent)) = movement
        && actions::activate(app, target.document())
    {
        let mut parameters = target.parameters();
        parameters["parent"] = json!(parent);
        app.run("explorer.move", parameters);
    }
    if let Some((target, command)) = action {
        if command == "activate" {
            actions::activate(app, target.document());
        } else {
            actions::invoke(app, &context, target, command);
        }
    }
}

/// Enter opens, F2 renames and Delete removes the selected graph or folder while the pointer is
/// over the Explorer; elsewhere those keys keep their graph shortcuts. True when a key was used
pub(crate) fn shortcuts(app: &mut MaterialCraftApp, ctx: &egui::Context) -> bool {
    if !app.ui.dock.is_visible(crate::docking::Panel::Explorer) || !app.ui.explorer.hovered || app.ui.explorer.rename.is_some() {
        return false;
    }

    let Some(target) = app.ui.explorer.selected.clone() else { return false };
    let command = match &target {
        ExplorerTarget::Graph { .. } => [(egui::Key::Enter, "graph.open"), (egui::Key::F2, "graph.rename"), (egui::Key::Delete, "graph.delete")]
            .into_iter()
            .find(|(key, _)| pressed(ctx, *key)),
        ExplorerTarget::Folder { .. } if !target.is_resources_root() => {
            [(egui::Key::F2, "folder.rename"), (egui::Key::Delete, "folder.remove")].into_iter().find(|(key, _)| pressed(ctx, *key))
        }
        _ => None,
    };
    let Some((key, command)) = command else { return false };
    ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
    actions::invoke(app, ctx, target, command);
    true
}

/// Match an unmodified Explorer shortcut key.
fn pressed(ctx: &egui::Context, key: egui::Key) -> bool {
    ctx.input(|input| input.modifiers.is_none() && input.key_pressed(key))
}

/// Draw the pending Explorer rename dialog.
pub(crate) fn dialogs(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    actions::rename_dialog(app, ctx);
}
