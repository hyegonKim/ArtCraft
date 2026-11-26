use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;
use std::time::Duration;

fn app() -> MaterialCraftApp {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let node = app.run("graph.node.add", json!({"def": "uniform", "params": {"color": [0.5, 0.75, 1.0, 1.0]}})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.node.add", json!({"def": "output", "connectFrom": {"node": node}, "params": {"usage": 1}})).unwrap();
    app
}

fn settle(h: &mut Harness<'static, MaterialCraftApp>, after: u64) {
    for _ in 0..300 {
        h.step();
        let app = h.state();
        if app.view3d.renders > after && !app.session.dirty && !app.cooker.busy() && !app.view3d.busy() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the normal-map preview did not finish rendering");
}

fn toggle(h: &mut Harness<'static, MaterialCraftApp>) {
    h.get_by_label("Display").click();
    h.run_steps(4);
    h.get_by_label("Flip normal map Y (DirectX)").click();
    h.run_steps(4);
}

#[test]
fn display_toggle_updates_the_session_and_rerenders_the_normal_map() {
    let mut h = Harness::builder().with_size(vec2(1400.0, 860.0)).build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app(),
    );
    settle(&mut h, 0);
    let before = h.state().view3d.shown.clone().unwrap();
    let rendered = h.state().view3d.renders;
    toggle(&mut h);
    assert!(h.state().session.view3d.normal_y_flip);
    settle(&mut h, rendered);
    assert_ne!(before.2, h.state().view3d.shown.as_ref().unwrap().2);
    // Close the open menu, then restore the OpenGL setting through the same pointer gesture.
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    let rendered = h.state().view3d.renders;
    toggle(&mut h);
    assert!(!h.state().session.view3d.normal_y_flip);
    settle(&mut h, rendered);
    assert_eq!(before.2, h.state().view3d.shown.as_ref().unwrap().2);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn normal_convention_snapshot() {
    let mut h = Harness::builder().with_size(vec2(1400.0, 860.0)).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app(),
    );
    settle(&mut h, 0);
    let rendered = h.state().view3d.renders;
    toggle(&mut h);
    settle(&mut h, rendered);
    h.get_by_label("Display").click();
    h.run_steps(4);
    assert!(h.state().session.view3d.normal_y_flip);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("normal-convention.png")).unwrap();
}
