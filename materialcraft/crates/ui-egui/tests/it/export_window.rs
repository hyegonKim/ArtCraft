//! The Export window: opened from the Explorer, its tabs, and Save settings against Close

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use materialcraft_ui_egui::export::ExportTab;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};

fn harness() -> Harness<'static, MaterialCraftApp> {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let mut harness = Harness::builder().with_size(vec2(1600.0, 960.0)).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    harness.run_steps(6);
    harness
}

fn open_from_the_graph_menu(harness: &mut Harness<'static, MaterialCraftApp>, item: &str) {
    let graph = harness.state().session.graph().unwrap().name.clone();
    harness.get(By::new().role(Role::TreeItem).label(graph.as_str())).click_secondary();
    harness.run_steps(3);
    // The Explorer toolbar has an Export outputs button too; the menu opens after it
    harness.get_all(By::new().role(Role::Button).label(item)).last().unwrap().click();
    harness.run_steps(3);
}

fn pattern_field(harness: &Harness<'static, MaterialCraftApp>) -> String {
    harness.state().ui.export_draft.as_ref().map(|draft| draft.pattern.clone()).unwrap_or_default()
}

#[test]
fn the_graph_menu_opens_the_window_on_the_matching_tab() {
    let mut harness = harness();

    open_from_the_graph_menu(&mut harness, "Export material...");
    assert!(harness.state().ui.windows.export);
    assert_eq!(harness.state().ui.export_tab, ExportTab::Mcmat);
    assert!(harness.query(By::new().role(Role::ComboBox).label("Normal orientation")).is_some(), "the #190 options are listed");

    harness.get(By::new().label("Close")).click();
    harness.run_steps(3);
    open_from_the_graph_menu(&mut harness, "Export outputs...");
    assert_eq!(harness.state().ui.export_tab, ExportTab::FromGraph);

    for tab in ["From graph", "Batch", "MaterialX", "MCMAT"] {
        harness.get(By::new().role(Role::Tab).label(tab)).click();
        harness.run_steps(2);
        assert!(harness.query(By::new().label("Save settings")).is_some(), "{tab}");
    }
    assert_eq!(harness.state().ui.export_tab, ExportTab::Mcmat);
}

#[test]
fn close_drops_the_windows_changes_and_save_settings_keeps_them() {
    let mut harness = harness();
    open_from_the_graph_menu(&mut harness, "Export outputs...");
    let saved = harness.state().ui.export.pattern.clone();

    harness.state_mut().ui.export_draft.as_mut().unwrap().pattern = "$(identifier)".into();
    harness.run_steps(2);
    assert_eq!(pattern_field(&harness), "$(identifier)");
    harness.get(By::new().label("Close")).click();
    harness.run_steps(3);
    assert!(!harness.state().ui.windows.export);
    assert_eq!(harness.state().ui.export.pattern, saved, "Close forgets the change");

    open_from_the_graph_menu(&mut harness, "Export outputs...");
    assert_eq!(pattern_field(&harness), saved, "the window starts from the saved settings");
    harness.state_mut().ui.export_draft.as_mut().unwrap().pattern = "$(identifier)".into();
    harness.run_steps(2);
    harness.get(By::new().label("Save settings")).click();
    harness.run_steps(3);
    assert_eq!(harness.state().ui.export.pattern, "$(identifier)");
    assert!(harness.state().ui.windows.export, "Save settings keeps the window open");
}

#[test]
fn checking_outputs_in_the_tree_updates_the_window_copy() {
    let mut harness = harness();
    open_from_the_graph_menu(&mut harness, "Export outputs...");
    let graph = harness.state().session.graph().unwrap().name.clone();

    harness.get(By::new().label("None")).click();
    harness.run_steps(2);
    let draft = harness.state().ui.export_draft.clone().unwrap();
    assert!(!draft.skipped_outputs.is_empty() && draft.skipped_outputs.iter().all(|key| key.starts_with(&format!("{graph}/"))));

    harness.get(By::new().role(Role::CheckBox).label(graph.as_str())).click();
    harness.run_steps(2);
    assert!(harness.state().ui.export_draft.as_ref().unwrap().skipped_outputs.is_empty(), "the graph's checkbox checks all its outputs");
}
