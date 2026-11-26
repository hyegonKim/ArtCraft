use super::*;
use materialcraft_engine::core::NodeId;
use serde_json::json;

/// Create a headless material editor for library interactions.
fn app() -> MaterialCraftApp {
    MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default())
}

/// Collect labels visible under the current library filter.
fn labels(app: &MaterialCraftApp) -> Vec<String> {
    let entries = app.session.library_entries();
    visible(app, &entries, &LibraryTarget::of(app)).into_iter().map(|entry| entry.label.clone()).collect()
}

/// Open project graphs remain separate from reusable library entries.
#[test]
fn open_package_graphs_are_explorer_resources_not_library_entries() {
    let mut app = app();
    let mut graph = materialcraft_engine::core::Graph::new("Reusable source");
    graph.add_node("output", [0.0; 2]);
    app.session.pkg.graphs.push(graph);
    app.ui.library_search = "Reusable source".into();
    assert!(labels(&app).is_empty());
    app.ui.library_search.clear();
    app.ui.library_category = "@package".into();
    assert!(labels(&app).is_empty(), "an unknown category lists nothing");
    app.ui.library_category.clear();
    assert!(labels(&app).iter().any(|label| label == "Uniform color"));
}

/// Category filtering includes descendants and favorite toggles update membership.
#[test]
fn categories_include_their_sub_categories_and_favorites_follow_the_star() {
    let mut app = app();
    app.ui.library_category = "Functions".into();
    let functions = labels(&app);
    app.ui.library_category = "Functions/Constant".into();
    let constants = labels(&app);
    assert!(!constants.is_empty() && constants.len() < functions.len());
    assert!(constants.iter().all(|label| functions.contains(label)));

    app.ui.library_category = FAVORITES.into();
    assert!(labels(&app).is_empty());
    toggle_favorite(&mut app, "node:blur");
    toggle_favorite(&mut app, "sbs://functions.sbs#Functions/Math/Pi");
    assert_eq!(labels(&app), ["Blur", "Pi"]);
    toggle_favorite(&mut app, "node:blur");
    assert_eq!(labels(&app), ["Pi"]);
}

/// Tiles and list rows stay alphabetical across categories, favorites and search.
#[test]
fn library_previews_are_alphabetical_in_every_view() {
    let mut app = app();
    for (category, search) in [("", ""), ("Graph items", ""), ("Atomic nodes", ""), ("Functions", ""), ("", "uniform")] {
        app.ui.library_category = category.into();
        app.ui.library_search = search.into();
        let found = labels(&app);
        assert!(found.len() > 1);
        let keys: Vec<_> = found.iter().map(|label| tr(label).to_lowercase()).collect();
        assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "{category}/{search}: {found:?}");
    }

    app.ui.library_show_missing = false;
    app.ui.library_show_other = false;
    let entries = app.session.library_entries();
    let target = LibraryTarget::of(&app);
    assert!(visible(&app, &entries, &target).iter().all(|entry| target.takes(entry)));
}

/// The FX-Map category contains its three processing nodes, without a Dot tile.
#[test]
fn fx_map_library_does_not_list_dot() {
    let mut app = app();
    app.ui.library_category = "FxMap nodes".into();
    let mut found = labels(&app);
    found.sort();
    assert_eq!(found, ["Iterate", "Quadrant", "Switch"]);
}

/// Both processor canvases accept samplers and keep material operations separate.
#[test]
fn processor_functions_take_operations_their_canvas_allows() {
    for definition in ["pixel_processor", "value_processor"] {
        let mut app = app();
        let owner = NodeId(app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        app.function_editor =
            Some(crate::function::FunctionEditor::new(app.session.graph, owner, crate::function::FunctionTarget::Parameter("function".into())));
        let target = LibraryTarget::of(&app);
        let entries = app.session.library_entries();
        let find = |id: &str| entries.iter().find(|entry| entry.id == id).unwrap();

        for id in ["function:add", "function:samplelum", "function:samplecol", "sbs://functions.sbs#Functions/Math/Pi"] {
            assert!(target.takes(find(id)), "{definition}: {id}");
        }
        assert!(!target.takes(find("node:blur")));
        assert!(target.refusal(find("node:blur")).is_some());
    }
}

/// Known categories have icons and compatible entries can be inserted.
#[test]
fn every_tree_category_has_an_icon_and_double_click_adds() {
    for (top, subs) in CATEGORIES {
        assert_ne!(category_icon(top), icon::FOLDER_SIMPLE, "{top}");
        for sub in *subs {
            assert_ne!(category_icon(&format!("{top}/{sub}")), icon::FOLDER_SIMPLE, "{top}/{sub}");
        }
    }

    let mut app = app();
    let before = app.session.graph().unwrap().nodes.len();
    let entries = app.session.library_entries();

    // A material graph takes the Blur node, not a function
    for id in ["node:blur", "sbs://functions.sbs#Functions/Math/Pi"] {
        let entry = entries.iter().find(|entry| entry.id == id).unwrap();
        if LibraryTarget::of(&app).takes(entry) {
            add_entry(&mut app, entry);
        }
    }
    assert_eq!(app.session.graph().unwrap().nodes.len(), before + 1, "only the entry the graph takes is added");
    let comments = app.session.graph().unwrap().comments.len();
    add_entry(&mut app, entries.iter().find(|entry| entry.id == "item:comment").unwrap());
    assert_eq!(app.session.graph().unwrap().comments.len(), comments + 1);
}

/// Dot is a graph item rather than a second entry among function operations.
#[test]
fn function_control_library_does_not_list_dot() {
    let mut app = app();
    app.ui.library_category = "Function nodes/Control".into();
    assert!(!labels(&app).iter().any(|label| label.to_lowercase().contains("dot")));
}

/// The shared Graph items remain available when hiding entries for other canvas types.
#[test]
fn graph_items_are_available_and_insertable_in_every_editor() {
    for kind in ["material", "function", "pixel_processor", "value_processor", "fx_map"] {
        let mut app = app();
        if kind == "function" {
            app.run("file.new", json!({"kind": "function"})).unwrap();
            app.function_editor = Some(crate::function::FunctionEditor::standalone(app.session.graph));
        } else if kind != "material" {
            let owner = NodeId(app.run("graph.node.add", json!({"def": kind})).unwrap()["id"].as_u64().unwrap());
            if kind == "fx_map" {
                app.pattern_editor = Some(crate::pattern::PatternEditor::new(app.session.graph, owner));
            } else {
                app.function_editor =
                    Some(crate::function::FunctionEditor::new(app.session.graph, owner, crate::function::FunctionTarget::Parameter("function".into())));
            }
        }
        app.graph.view = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        app.ui.library_category = "Graph items".into();
        app.ui.library_show_other = false;
        assert_eq!(labels(&app), ["Comment", "Dot node", "Frame", "Pin"], "{kind}");
        let entries = app.session.library_entries();
        let target = LibraryTarget::of(&app);
        for entry in entries.iter().filter(|entry| entry.category == "Graph items") {
            assert!(target.takes(entry), "{kind}: {}", entry.label);
            add_entry(&mut app, entry);
        }
        let items = crate::annotation::target(&app).items(&app.session).unwrap();
        assert_eq!((items.comments.len(), items.frames.len(), items.pins.len()), (1, 1, 1), "{kind}");
    }
}
