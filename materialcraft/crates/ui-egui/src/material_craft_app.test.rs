#[cfg(test)]
mod lib_tests {
    use crate::MaterialCraftApp;
    use crate::state::UiState;
    use materialcraft_engine::sample_session;
    use serde_json::json;

    #[test]
    fn copy_and_cut_fill_the_system_clipboard_in_material_and_function_canvases() {
        use crate::function::{FunctionEditor, FunctionTarget};
        use materialcraft_engine::core::{FunctionGraph, NodeId, Value};

        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let owner = NodeId(app.run("graph.node.add", json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap());
        let target = materialcraft_engine::FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
        app.run("graph.function.set", json!({"target": target, "function": FunctionGraph::constant(Value::Float(0.5))})).unwrap();
        let context = egui::Context::default();

        crate::menus::invoke(&mut app, &context, "edit.selectAll");
        crate::menus::invoke(&mut app, &context, "edit.copy");
        let material = app.system_clipboard.take().unwrap();
        app.function_editor = Some(FunctionEditor::new(app.session.graph, owner, FunctionTarget::Parameter("function".into())));
        crate::menus::invoke(&mut app, &context, "edit.selectAll");
        crate::menus::invoke(&mut app, &context, "edit.cut");
        let function = app.system_clipboard.take().unwrap();

        assert_eq!(serde_json::from_str::<serde_json::Value>(&material).unwrap()["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&function).unwrap(), serde_json::to_value(&app.session.embedded_clipboard).unwrap());
        assert!(app.session.embedded_clipboard.is_some());
    }

    /// A frame which does not draw Explorer clears hover state left by its previous frame.
    #[test]
    fn review_hidden_explorer_drawing_clears_hover() {
        let mut app = MaterialCraftApp::headless(sample_session(), UiState::default());
        app.fonts_ready = true;
        app.ui.explorer.hovered = true;
        app.ui.dock.close(crate::docking::Panel::Explorer);
        let context = egui::Context::default();

        context.run_ui(egui::RawInput::default(), |ui| app.draw(ui)).textures_delta.clear();

        assert!(!app.ui.explorer.hovered);
    }

    #[test]
    fn run_reports_errors_as_toasts_and_keeps_the_session() {
        let mut app = MaterialCraftApp::headless(sample_session(), UiState::default());
        assert!(app.run("no.such.command", json!({})).is_none());
        assert!(app.toasts.iter().any(|t| t.error));
        assert!(app.run("graph.node.add", json!({"def": "levels"})).is_some());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod close_order_tests {
    use crate::MaterialCraftApp;
    use crate::state::UiState;

    fn close_frame(app: &mut MaterialCraftApp) -> bool {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
        let mut canceled = false;
        ctx.run_ui(input, |ui| {
            app.step_then(ui.ctx(), |ctx| {
                canceled = ctx.viewport(|viewport| viewport.commands.contains(&egui::ViewportCommand::CancelClose));
                false
            });
        })
        .textures_delta
        .clear();
        canceled
    }

    #[test]
    fn the_quit_hook_runs_after_the_close_guard() {
        let mut held = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
        held.fonts_ready = true;
        held.session.modified = true;
        assert!(close_frame(&mut held), "the guard canceled the close before the hook runs");

        let mut clean = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
        clean.fonts_ready = true;
        clean.session.modified = false;
        assert!(!close_frame(&mut clean), "nothing to hold: the hook sees a close to take over");
    }
}

#[cfg(test)]
mod unsaved_state_tests {
    use crate::unsaved::{Choice, Guarded, Step, answer, guard};
    use crate::{MaterialCraftApp, state::UiState};

    fn app(modified: bool) -> MaterialCraftApp {
        let mut s = materialcraft_engine::sample_session();
        s.modified = modified;
        MaterialCraftApp::headless(s, UiState::default())
    }

    #[test]
    fn closing_all_over_unsaved_work_waits_for_an_answer() {
        let mut a = app(true);
        let before = a.session.pkg.name.clone();
        guard(&mut a, Guarded::Menu { id: "ui.closeAll".into() });
        assert!(a.ui.windows.unsaved.is_some());
        assert_eq!(a.session.pkg.name, before, "nothing was replaced yet");
        // The control channel can inspect the held action
        let v = serde_json::to_value(&a.ui).unwrap();
        assert_eq!(v["windows"]["unsaved"]["kind"], "menu");

        assert_eq!(answer(&mut a.ui.windows.unsaved, Choice::Cancel), Step::Stop);
        assert!(a.session.modified);
        assert_eq!(a.session.pkg.name, before);
    }

    #[test]
    fn closing_all_over_a_clean_package_just_happens() {
        let mut a = app(false);
        guard(&mut a, Guarded::Menu { id: "ui.closeAll".into() });

        assert!(a.ui.windows.unsaved.is_none());
        assert!(a.session.pkg.graphs.is_empty());
        assert!(a.session.open_packages().is_empty());
    }

    #[test]
    fn a_pending_dialog_is_not_restored_from_saved_state() {
        let mut a = app(true);
        guard(&mut a, Guarded::Quit);
        let saved = serde_json::to_string(&a.ui).unwrap();
        let back: UiState = serde_json::from_str(&saved).unwrap();
        assert!(back.windows.unsaved.is_none());
    }
}
