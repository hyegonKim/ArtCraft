use super::{ExplorerState, ExplorerTarget, context_menu, tree_row};
use crate::graph_drag_payload::GraphDragPayload;
use egui::Ui;
use egui_phosphor::regular as icon;
use materialcraft_engine::{
    Session,
    core::{NodeId, Package},
};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) struct ExplorerTree<'a> {
    pub session: &'a Session,
    pub state: &'a mut ExplorerState,
    pub action: Option<(ExplorerTarget, &'static str)>,
    pub opened_node: Option<(u64, usize, NodeId)>,
    pub assignment: Option<Value>,
    pub movement: Option<(ExplorerTarget, String)>,
    pub inspected_graph: Option<materialcraft_engine::GraphTarget>,
    pub inspected_folder: Option<(u64, String)>,
}

impl ExplorerTree<'_> {
    pub fn package(&mut self, ui: &mut Ui, document: u64, package: &Package, path: Option<&str>, modified: bool) {
        let warnings: Vec<_> = package
            .graphs
            .iter()
            .map(|graph| {
                materialcraft_engine::function_inspection::warnings(graph).into_iter().flat_map(|(_, messages)| messages).collect::<Vec<_>>().join("\n")
            })
            .collect();
        let package_warning = warnings.iter().filter(|warning| !warning.is_empty()).cloned().collect::<Vec<_>>().join("\n");
        let target = ExplorerTarget::Package { document };
        let name = path.and_then(|path| path.rsplit(['/', '\\']).next()).unwrap_or(&package.name);
        let name = format!("{name}{}", if modified { "*" } else { "" });
        let folders = package.folder_paths();

        // The implicit Resources group is hidden until it contains assets or subfolders
        let has_children =
            !package.graphs.is_empty() || !package.resources.is_empty() || folders.iter().any(|folder| !folder.eq_ignore_ascii_case("Resources"));
        let mut header = |ui: &mut Ui| {
            let response = self.row(ui, &target, package, &name, icon::PACKAGE, &package_warning, path.is_some());
            if let Some(path) = path {
                response.on_hover_text(path);
            }
        };
        if !has_children {
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                header(ui);
            });
            return;
        }

        let id = ui.make_persistent_id((document, "package"));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true).show_header(ui, header).body(|ui| {
            self.level(ui, document, package, &folders, "", &warnings, 0);
        });
    }

    fn level(&mut self, ui: &mut Ui, document: u64, package: &Package, folders: &BTreeSet<String>, parent: &str, warnings: &[String], depth: usize) {
        // Imported paths are bounded too, but UI inspection can supply arbitrary state
        if depth > 64 {
            return;
        }

        for folder in folders.iter().filter(|path| path.rsplit_once('/').map_or("", |(parent, _)| parent) == parent) {
            if folder.eq_ignore_ascii_case("Resources")
                && !package.resources.iter().any(|resource| resource.folder == *folder || resource.folder.starts_with(&format!("{folder}/")))
                && !folders.iter().any(|path| path.starts_with(&format!("{folder}/")))
            {
                continue;
            }

            let target = ExplorerTarget::Folder { document, path: folder.clone() };
            let name = folder.rsplit('/').next().unwrap_or(folder);
            let id = ui.make_persistent_id((document, "folder", folder));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    self.row(ui, &target, package, name, icon::FOLDER, "", false);
                })
                .body(|ui| self.level(ui, document, package, folders, folder, warnings, depth + 1));
        }
        for resource in package.resources.iter().filter(|resource| resource.folder == parent) {
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                let target = ExplorerTarget::Resource { document, name: resource.name.clone() };
                self.row(ui, &target, package, &resource.name, icon::IMAGE, if resource.bytes.is_empty() { "Missing resource" } else { "" }, false);
            });
        }
        for (index, graph) in package.graphs.iter().enumerate().filter(|(_, graph)| graph.folder == parent) {
            let target = ExplorerTarget::Graph { document, index };
            let symbol = if graph.function.is_some() { icon::FUNCTION } else { icon::TREE_STRUCTURE };
            let warning = warnings.get(index).map(String::as_str).unwrap_or("");
            let mut ports =
                graph.nodes.iter().filter(|node| matches!(node.def.as_str(), "output" | "input_color" | "input_grayscale" | "input_value")).peekable();
            if graph.function.is_some() || ports.peek().is_none() {
                ui.horizontal(|ui| {
                    ui.add_space(ui.spacing().indent);
                    self.row(ui, &target, package, &graph.name, symbol, warning, false);
                });
                continue;
            }

            let id = ui.make_persistent_id((document, "graph", index));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
                .show_header(ui, |ui| {
                    self.row(ui, &target, package, &graph.name, symbol, warning, false);
                })
                .body(|ui| {
                    ui.indent("graph-ports", |ui| {
                        for node in ports {
                            ui.push_id(node.id, |ui| {
                                let name = crate::graph::node_label(node);
                                let target = ExplorerTarget::Node { document, graph: index, node: node.id };
                                let selected = self.state.selected.as_ref() == Some(&target);
                                let symbol = if node.def == "input_value" { icon::FUNCTION } else { icon::IMAGE };
                                let response = tree_row::show(ui, &name, symbol, selected, None);
                                response.context_menu(|ui| {
                                    if document == self.session.document_id
                                        && let Some(parameters) = crate::material_preview_menu::show(ui, self.session, index, node.id)
                                    {
                                        self.assignment = Some(parameters);
                                    }
                                });
                                if response.clicked() || response.secondary_clicked() {
                                    self.state.selected = Some(target);
                                }
                                if response.double_clicked() {
                                    self.opened_node = Some((document, index, node.id));
                                }
                            });
                        }
                    });
                });
        }
    }

    fn row(&mut self, ui: &mut Ui, target: &ExplorerTarget, package: &Package, name: &str, symbol: &str, warning: &str, reload: bool) -> egui::Response {
        let selected = self.state.selected.as_ref().map_or_else(
            || matches!(target, ExplorerTarget::Graph { document, index } if *document == self.session.document_id && *index == self.session.graph),
            |selected| selected == target,
        );
        let response = tree_row::show(ui, name, symbol, selected, (!warning.is_empty()).then_some(warning));
        if response.clicked() || response.secondary_clicked() {
            self.state.selected = Some(target.clone());
            if let ExplorerTarget::Graph { document, index } = target {
                self.inspected_graph = Some(materialcraft_engine::GraphTarget { document: *document, graph: *index });
            }
            if let ExplorerTarget::Folder { document, path } = target {
                self.inspected_folder = Some((*document, path.clone()));
            }
        }
        if response.double_clicked() && matches!(target, ExplorerTarget::Graph { .. }) {
            self.action = Some((target.clone(), "graph.open"));
        }
        if response.double_clicked() && matches!(target, ExplorerTarget::Package { .. } | ExplorerTarget::Folder { .. }) {
            self.action = Some((target.clone(), "activate"));
        }
        response.context_menu(|ui| {
            let outputs = match target {
                ExplorerTarget::Graph { index, .. } => package.graphs.get(*index).is_some_and(|graph| graph.function.is_none()),
                _ => package.graphs.iter().any(|graph| graph.function.is_none()),
            };
            let dependencies = package.dependencies.keys().any(|path| super::actions::is_package_file(path));
            let state =
                super::explorer_menu_state::ExplorerMenuState { paste: self.session.explorer_clipboard.is_some(), saved: reload, dependencies, outputs };
            if let Some(command) = context_menu::show(ui, target, &state) {
                self.action = Some((target.clone(), command));
            }
        });
        if response.drag_started() {
            match target {
                ExplorerTarget::Graph { document, index } => {
                    if let Some(graph) = package.graphs.get(*index) {
                        egui::DragAndDrop::set_payload(ui.ctx(), GraphDragPayload { document: *document, name: graph.sbs_path() });
                    }
                }
                ExplorerTarget::Folder { .. } if !target.is_resources_root() => egui::DragAndDrop::set_payload(ui.ctx(), target.clone()),
                ExplorerTarget::Resource { .. } => egui::DragAndDrop::set_payload(ui.ctx(), target.clone()),
                _ => {}
            }
        }
        if !matches!(target, ExplorerTarget::Resource { .. })
            && let Some(parent) = target.folder(package)
        {
            if !target.is_resources()
                && let Some(payload) = crate::widgets::dnd_release::<GraphDragPayload>(&response)
                && payload.document == target.document()
                && let Some((index, _)) = package.graph_by_name(&payload.name)
            {
                self.movement = Some((ExplorerTarget::Graph { document: payload.document, index }, parent.clone()));
            }
            if let Some(payload) = crate::widgets::dnd_release::<ExplorerTarget>(&response)
                && payload.document() == target.document()
            {
                self.movement = Some((payload.as_ref().clone(), parent));
            }
        }
        response
    }
}
