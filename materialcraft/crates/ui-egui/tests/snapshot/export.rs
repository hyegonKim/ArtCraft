use super::{By, MaterialCraftApp, Queryable, UiState, harness, save, settle};
use egui::accesskit::Role;

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn export_window_tabs() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);

    let graph = harness.state().session.graph().unwrap().name.clone();
    harness.get(By::new().role(Role::TreeItem).label(graph.as_str())).click_secondary();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "export-graph-menu");
    harness.get_all(By::new().role(Role::Button).label("Export material...")).last().unwrap().click();
    settle(&mut harness, 4);

    for (tab, name) in [("MCMAT", "export-mcmat"), ("MaterialX", "export-materialx"), ("From graph", "export-from-graph"), ("Batch", "export-batch")] {
        harness.get(By::new().role(Role::Tab).label(tab)).click();
        settle(&mut harness, 4);
        save(&mut harness, &directory, name);
    }

    harness.get(By::new().role(Role::Tab).label("From graph")).click();
    settle(&mut harness, 3);
    harness.get(By::new().role(Role::ComboBox).label("Format")).click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "export-formats");
}
