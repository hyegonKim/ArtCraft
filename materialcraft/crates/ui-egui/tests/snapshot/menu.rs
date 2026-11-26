use super::{By, MaterialCraftApp, NodeT, Queryable, UiState, harness, save, settle};
use egui::accesskit::{Role, Toggled};

/// A menu toggle keeps the row neutral and exposes its state with a checkmark.
fn check_menu_toggle(menu: &str, label: &str) {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let button = |label| By::new().role(Role::Button).label(label);
    let anchor = if menu == "Tools" { "Compute graph" } else { "Reset layout" };
    let mut fills = Vec::new();
    let mut marks = Vec::new();

    for checked in [true, false] {
        harness.get(button(menu)).click();
        settle(&mut harness, 3);
        harness.event(egui::Event::PointerMoved(egui::pos2(800.0, 250.0)));
        settle(&mut harness, 3);
        let menu_items = harness.get(button(anchor)).parent().unwrap();
        let item = menu_items.get(button(label));
        assert_eq!(item.accesskit_node().toggled(), Some(if checked { Toggled::True } else { Toggled::False }));
        let rect = item.rect();
        let image = harness.render().unwrap();
        fills.push(*image.get_pixel((rect.right() - 8.0) as u32, rect.center().y as u32));
        let mut mark = Vec::new();
        for y in (rect.center().y as u32 - 5)..(rect.center().y as u32 + 5) {
            for x in (rect.left() as u32 + 3)..(rect.left() as u32 + 19) {
                mark.push(*image.get_pixel(x, y));
            }
        }
        marks.push(mark);
        save(&mut harness, &directory, &format!("menu-{}-{}", menu.to_lowercase(), if checked { "checked" } else { "unchecked" }));
        harness.get(button(anchor)).parent().unwrap().get(button(label)).click();
        settle(&mut harness, 3);
    }

    assert_eq!(fills[0], fills[1], "checked items must not have a persistent row highlight");
    assert_ne!(marks[0], marks[1], "the checkmark must show the toggle state");
}

/// Tools uses a checkmark for snapping while the rest of the row stays neutral.
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn tools_menu_uses_checkmarks_for_toggles() {
    check_menu_toggle("Tools", "Snap to grid");
}

/// Windows uses the same checkmark treatment for visible panels.
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn windows_menu_uses_checkmarks_for_panels() {
    check_menu_toggle("Windows", "Explorer");
}
