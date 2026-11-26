//! Mouse-wheel zoom and pan preference tests on the canvas.

use egui::{Event, Modifiers, MouseWheelUnit, vec2};
use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn ctrl_mouse_wheel_zooms_canvas() {
    let mut s = Session::new();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    let mut u = Ui::new(s);
    let before = u.app.session.ds().unwrap().view.zoom;
    let center = u.app.canvas.rect.center();
    u.frame_with(
        vec![
            Event::PointerMoved(center),
            Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                source: egui::MouseWheelSource::Wheel,
                delta: vec2(0.0, 2.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers { ctrl: true, command: true, ..Modifiers::NONE },
            },
        ],
        Modifiers { ctrl: true, command: true, ..Modifiers::NONE },
    );
    let after = u.app.session.ds().unwrap().view.zoom;
    assert_ne!(after, before);
}
