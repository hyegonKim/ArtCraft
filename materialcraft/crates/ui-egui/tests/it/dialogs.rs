//! Home and export dialogs start centered, but their title bars must allow movement.

use egui::{Event, Modifiers, PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};

type State = (MaterialCraftApp, Rect);

fn harness(id: &'static str, render: bool) -> Harness<'static, State> {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.ui.windows.home = id == "home";
    app.ui.windows.export = id == "export";
    let builder = Harness::builder().with_size(vec2(1600.0, 960.0)).with_pixels_per_point(1.0);
    let builder = if render { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        move |ui, (app, rect): &mut State| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
            *rect = ui.ctx().memory(|m| m.area_rect(egui::Id::unique(id))).unwrap_or(Rect::NOTHING);
        },
        (app, Rect::NOTHING),
    );
    h.run_steps(10);
    h
}

fn drag_title(h: &mut Harness<'static, State>) {
    let from = h.state().1.min + vec2(120.0, 12.0);
    h.event(Event::PointerMoved(from));
    h.step();
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    for step in 1..=8 {
        h.event(Event::PointerMoved(from + vec2(10.0, 7.5) * step as f32));
        h.step();
    }
    h.event(Event::PointerButton { pos: from + vec2(80.0, 60.0), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(10);
}

#[test]
fn home_and_export_can_be_dragged_and_keep_their_position() {
    for id in ["home", "export"] {
        let mut h = harness(id, false);
        let before = h.state().1;
        assert!(before.is_finite());
        assert!(before.center().distance(pos2(800.0, 480.0)) < 3.0, "{id}: initial rectangle {before:?}");
        drag_title(&mut h);
        let moved = h.state().1;
        assert!(moved.min.distance(before.min + vec2(80.0, 60.0)) < 8.0, "{id}: did not follow the drag: {before:?} -> {moved:?}");
        // Later frames and reopening the same dialog must not snap it back to the center.
        h.state_mut().0.ui.windows.home = false;
        h.state_mut().0.ui.windows.export = false;
        h.run_steps(3);
        h.state_mut().0.ui.windows.home = id == "home";
        h.state_mut().0.ui.windows.export = id == "export";
        h.run_steps(10);
        assert!(h.state().1.min.distance(moved.min) < 1.0, "{id}: reopening lost the dragged position");
        h.set_size(vec2(1500.0, 900.0));
        h.run_steps(10);
        assert!(h.state().1.min.distance(moved.min) < 1.0, "{id}: resizing recentered the dialog");
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn dialog_drag_snapshots() {
    let dir = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    for id in ["home", "export"] {
        let mut h = harness(id, true);
        h.render().unwrap().save(dir.join(format!("{id}-centered.png"))).unwrap();
        drag_title(&mut h);
        h.render().unwrap().save(dir.join(format!("{id}-moved.png"))).unwrap();
    }
}
