//! Changing interface scale must not move its slider under a held pointer.
use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::{
    Harness,
    kittest::{By, Queryable},
};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};

fn harness(scale: f32, render: bool) -> Harness<'static, MaterialCraftApp> {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.windows.preferences = true;
    app.ui.prefs.ui_scale = scale;
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_pixels_per_point(1.0).with_step_dt(1.0 / 60.0);
    let builder = if render { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    h.run_steps(10);
    h
}

// Keep the physical window and pointer fixed, just as a native window does when zoom changes.
fn point(h: &mut Harness<'static, MaterialCraftApp>, physical: Pos2, pressed: Option<bool>) {
    let zoom = h.ctx.zoom_factor();
    let pos = physical / zoom;
    h.input_mut().screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 960.0) / zoom));
    h.event(Event::PointerMoved(pos));
    if let Some(pressed) = pressed {
        h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.step();
}

fn drag_scale(h: &mut Harness<'static, MaterialCraftApp>, fraction: f32) {
    let initial = h.ctx.zoom_factor();
    let track = h.get(By::new().role(egui::accesskit::Role::Slider).label("Interface scale")).rect();
    let start = track.center() * initial;
    let target = pos2(track.left() + track.width() * fraction, track.center().y) * initial;
    point(h, start, None);
    point(h, start, Some(true));
    point(h, target, None);
    let chosen = h.state().ui.prefs.ui_scale;
    assert!((chosen - initial).abs() > 0.1, "the drag did not change the setting");
    for _ in 0..15 {
        point(h, target, None);
        assert!((h.state().ui.prefs.ui_scale - chosen).abs() < 0.001, "scale drifted under a stationary pointer");
        assert!((h.ctx.zoom_factor() - initial).abs() < 0.001, "zoom changed while dragging");
    }
    point(h, target, Some(false));
    h.run_steps(5);
    assert!((h.state().ui.prefs.ui_scale - chosen).abs() < 0.001, "release changed the chosen value");
    assert!((h.ctx.zoom_factor() - chosen).abs() < 0.001, "release did not apply the chosen scale");
}

#[test]
fn scale_drags_are_stable_and_apply_on_release() {
    for (initial, fraction) in [(1.0, 0.7), (1.0, 0.1), (1.5, 0.2)] {
        let mut h = harness(initial, false);
        drag_scale(&mut h, fraction);
    }
}

#[test]
fn scale_track_click_applies_after_release() {
    let mut h = harness(1.0, false);
    let track = h.get(By::new().role(egui::accesskit::Role::Slider).label("Interface scale")).rect();
    let target = pos2(track.left() + track.width() * 0.8, track.center().y);
    point(&mut h, target, None);
    point(&mut h, target, Some(true));
    assert_eq!(h.ctx.zoom_factor(), 1.0);
    point(&mut h, target, Some(false));
    h.run_steps(5);
    assert!(h.state().ui.prefs.ui_scale > 1.5);
    assert!((h.ctx.zoom_factor() - h.state().ui.prefs.ui_scale).abs() < 0.001);
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn scale_drag_screenshot() {
    let mut h = harness(1.0, true);
    drag_scale(&mut h, 0.7);
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    h.render().unwrap().save(dir.join("interface-scale-after-drag.png")).unwrap();
}
