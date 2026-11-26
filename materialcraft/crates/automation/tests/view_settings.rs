use materialcraft_automation::headless::{Filesystem, Headless, RenderOptions};
use materialcraft_engine::new_session;
use serde_json::json;

#[test]
fn rejected_view_command_preserves_the_rendered_scene() {
    let mut h = Headless::new(new_session(), Filesystem::Denied);
    let options = RenderOptions { width: Some(64), height: Some(48), ..RenderOptions::default() };
    h.command_run("view.3d", json!({"scene": "sphere", "environment": "sunset"})).unwrap();
    let before = h.render_png(&options).unwrap().png;
    assert!(h.command_run("view.3d", json!({"scene": "cube", "environment": "invalid"})).is_err());
    assert!(before == h.render_png(&options).unwrap().png, "rejected settings changed the rendered scene");
    // Confirm that valid settings still change the rendered output rather than returning a stale image.
    h.command_run("view.3d", json!({"scene": "cube", "environment": "night"})).unwrap();
    assert_ne!(before, h.render_png(&options).unwrap().png);
}
