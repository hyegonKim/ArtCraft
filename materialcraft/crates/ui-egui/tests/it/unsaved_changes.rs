use egui::{Rect, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use materialcraft_ui_egui::{
    MaterialCraftApp, UiState,
    unsaved::{self, Guarded},
};

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn unsaved_dialog_snapshot_and_cancel() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.session.modified = true;
    unsaved::guard(&mut app, Guarded::Menu { id: "ui.closeAll".into() });
    let mut h = Harness::builder().with_size(vec2(1600.0, 960.0)).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(10);
    let rect = h.ctx.memory(|m| m.area_rect(egui::Id::unique("unsaved-changes"))).unwrap_or(Rect::NOTHING);
    assert!(rect.is_finite() && rect.width() > 100.0);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("unsaved-changes.png")).unwrap();
    h.get_by_label("Cancel").click();
    h.run_steps(5);
    assert!(h.state().ui.windows.unsaved.is_none());
    assert!(h.state().session.modified);
}
