use super::{By, MaterialCraftApp, Queryable, UiState, harness, save, settle, vec2};
use egui::accesskit::Role;

/// Find a category by its accessible tree label.
fn tree(label: &str) -> By<'_> {
    By::new().role(Role::TreeItem).label(label)
}

/// Find an entry by its accessible list-item label.
fn tile(label: &str) -> By<'_> {
    By::new().role(Role::ListItem).label(label)
}

/// Render categories, search and favorites while exercising a library drag.
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn library_tree_shows_every_category_and_adds_what_the_graph_takes() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);

    for label in [
        "Favorites",
        "Graph items",
        "Atomic nodes",
        "FxMap nodes",
        "Function nodes",
        "Texture generators",
        "Filters",
        "Material filters",
        "Mesh-based generators",
        "Spline & Path tools",
        "MCF functions",
        "Functions",
        "3D view",
        "PBR materials",
        "Values",
    ] {
        assert!(harness.query(tree(label)).is_some(), "{label}");
    }
    assert!(harness.query(tree("SDF functions")).is_none(), "the category is MCF functions now");

    // Graph items, then the atomic nodes with their previews
    harness.get(tree("Graph items")).click();
    settle(&mut harness, 3);
    for label in ["Comment", "Frame", "Pin", "Dot node"] {
        assert!(harness.query(tile(label)).is_some(), "{label}");
    }
    harness.get(tree("Atomic nodes")).click();
    settle(&mut harness, 12);
    for label in ["Blend", "Blur", "Levels", "Output", "Input color"] {
        assert!(harness.query(tile(label)).is_some(), "{label}");
    }
    save(&mut harness, &directory, "library-atomic-nodes");

    // Dragging a tile adds it where it drops
    let before = harness.state().session.graph().unwrap().nodes.len();
    let start = harness.get(tile("Blur")).rect().center();
    let end = harness.state().graph.view.center() + vec2(120.0, 80.0);
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    for position in [start + vec2(12.0, 0.0), end] {
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
    }
    harness.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    settle(&mut harness, 5);
    let graph = harness.state().session.graph().unwrap();
    assert_eq!(graph.nodes.len(), before + 1);
    assert_eq!(graph.nodes.last().unwrap().def, "blur");

    // Procedural texture generators appear under Noises; unpopulated categories say so
    harness.get(tree("Texture generators")).click();
    settle(&mut harness, 3);
    harness.get(tree("Noises")).click();
    settle(&mut harness, 4);
    assert!(harness.query(tile("Perlin noise")).is_some());
    harness.get(tree("Texture generators")).click();
    settle(&mut harness, 2);
    harness.get(tree("Spline & Path tools")).click();
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label("Nothing in this category yet")).is_some());
    harness.get(tree("Spline & Path tools")).click();
    settle(&mut harness, 2);

    // Opening a category shows its sub-categories; functions sort into theirs by folder
    harness.get(tree("Functions")).click();
    settle(&mut harness, 3);
    harness.get(tree("Constant")).click();
    settle(&mut harness, 4);
    for label in ["Pi", "2 Pi"] {
        assert!(harness.query(tile(label)).is_some(), "{label}");
    }
    save(&mut harness, &directory, "library-functions-constant");
    harness.get(tree("Functions")).click();
    settle(&mut harness, 2);
    harness.get(tree("Functions")).click();
    settle(&mut harness, 2);
    assert!(!harness.state().ui.library_expanded.contains("Functions"));

    for (top, subs) in [
        ("Filters", &["Adjustments", "Blending", "Blurs", "Channels", "Effects", "Normal map", "Tiling", "Transforms"][..]),
        ("Function nodes", &["Constant", "Vector", "Variables", "Samplers", "Cast", "Operator", "Logical", "Comparison", "Function", "Control"][..]),
        ("MCF functions", &["Primitives", "Operators", "Transforms"][..]),
        ("PBR materials", &["Bricks", "Concrete", "Fabric", "Ground", "Metal", "Paper", "Pavement", "Tiles", "Wood", "Stone"][..]),
    ] {
        harness.get(tree(top)).click();
        settle(&mut harness, 3);
        for sub in subs {
            assert!(harness.query_all(tree(sub)).next().is_some(), "{top}/{sub}");
        }
        if top == "PBR materials" {
            save(&mut harness, &directory, "library-categories-open");
        }

        // Clicking the selected, open category closes it again
        harness.get(tree(top)).click();
        settle(&mut harness, 2);
        assert!(!harness.state().ui.library_expanded.contains(top), "{top}");
    }

    // A selected tile goes into Favorites with the star
    harness.get(tree("Atomic nodes")).click();
    settle(&mut harness, 3);
    harness.get(tile("Blend")).click();
    settle(&mut harness, 2);
    harness.get(By::new().role(Role::Button).label("Add to Favorites")).click();
    settle(&mut harness, 2);
    harness.get(tree("Favorites")).click();
    settle(&mut harness, 3);
    assert!(harness.query(tile("Blend")).is_some());
    assert_eq!(harness.state().ui.library_favorites, ["node:blend"]);
    save(&mut harness, &directory, "library-favorites");

    // Search finds entries across the whole library, what the graph takes first
    harness.state_mut().ui.library_search = "blur".into();
    settle(&mut harness, 4);
    assert!(harness.query(tile("Blur")).is_some());
    save(&mut harness, &directory, "library-search");
}
