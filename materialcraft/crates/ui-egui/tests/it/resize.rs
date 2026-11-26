//! Resizing panels and the window must stay smooth: the 3D view does not re-render on every
//! step of a resize, and its full-quality frame renders off the UI thread once the size rests.

use std::time::Duration;

use egui::vec2;
use egui_kittest::Harness;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};

fn harness(w: f32, h: f32) -> Harness<'static, MaterialCraftApp> {
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    Harness::builder().with_size(vec2(w, h)).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    )
}

/// Step until the cook and the 3D view's background render are done (or give up).
fn settle(h: &mut Harness<'static, MaterialCraftApp>) {
    for _ in 0..400 {
        h.step();
        let app = h.state();
        if !app.cooker.busy() && !app.session.dirty && !app.view3d.busy() && app.view3d.renders > 0 {
            // A few more frames: the settle delay may still be running after the last change.
            for _ in 0..20 {
                h.step();
            }
            if !h.state().view3d.busy() {
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the 3D view never settled");
}

#[test]
fn a_resize_does_not_render_the_3d_view_on_every_frame() {
    let mut h = harness(1400.0, 860.0);
    settle(&mut h);
    let before = h.state().view3d.renders;
    let shown = h.state().view3d.shown.as_ref().map(|(w, ht, _)| (*w, *ht));
    // A drag: the window (and with it every panel) gets narrower on each of 40 frames.
    for i in 0..40 {
        h.set_size(vec2(1400.0 - 4.0 * i as f32, 860.0));
        h.step();
    }
    assert_eq!(h.state().view3d.renders, before, "the 3D view rendered while it was being resized");
    assert!(!h.state().view3d.busy(), "a render started while the view was being resized");
    // Released: one full-quality frame at the new size, in the background.
    settle(&mut h);
    assert_eq!(h.state().view3d.renders, before + 1);
    let now = h.state().view3d.shown.as_ref().map(|(w, ht, _)| (*w, *ht));
    assert!(now.is_some() && now != shown, "the 3D view kept the old frame: {shown:?} -> {now:?}");
}
