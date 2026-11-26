use serde_json::{Value, json};

use super::*;

/// One 16 px graph, New_graph, with two connected outputs: base_color (base color usage) and height
fn app() -> MaterialCraftApp {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
    app.run("graph.size", json!({"width": 16}));
    for (identifier, usage) in [("base_color", "baseColor"), ("height", "height")] {
        let usage = materialcraft_engine::core::ComponentUsage::index(usage);
        let source = app.run("graph.node.add", json!({"def": "uniform"})).unwrap()["id"].as_u64().unwrap();
        app.run("graph.node.add", json!({"def": "output", "params": {"identifier": identifier, "usage": usage}, "connectFrom": {"node": source}})).unwrap();
    }
    app
}

fn temp(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("mc-export-window-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

fn files(directory: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .map_or_else(|_| Vec::new(), |entries| entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect());
    names.sort();
    names
}

#[test]
fn selection_names_the_checked_outputs_and_their_color_spaces() {
    let app = app();
    let graph = app.session.graph().unwrap();
    let mut settings = ExportSettings::default();

    let (outputs, color_spaces) = settings.selection(graph).unwrap();
    assert_eq!(outputs, Value::Null, "with every output checked, file.export writes them all");
    assert_eq!(color_spaces, json!({"base_color": "srgb", "height": "raw"}), "unpicked outputs follow their usage");

    settings.skipped_outputs.insert(output_key("New_graph", "height"));
    settings.color_spaces.insert(output_key("New_graph", "base_color"), materialcraft_engine::ColorSpace::Linear);
    assert_eq!(settings.selection(graph).unwrap(), (json!(["base_color"]), json!({"base_color": "linear"})));

    settings.include_all(graph, false);
    assert_eq!(settings.selection(graph), None);
    settings.include_all(graph, true);
    assert!(settings.skipped_outputs.is_empty());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn from_graph_writes_only_the_checked_outputs() {
    let mut app = app();
    let directory = temp("from-graph");
    let mut settings = ExportSettings { destination: directory.display().to_string(), pattern: "$(identifier)".into(), ..ExportSettings::default() };
    settings.skipped_outputs.insert(output_key("New_graph", "height"));
    app.ui.export_tab = ExportTab::FromGraph;

    assert!(export(&mut app, &settings));

    assert_eq!(files(&directory), ["base_color.png"]);
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn batch_writes_every_material_graph_at_the_output_size() {
    let mut app = app();
    app.run("file.new", json!({"name": "Second", "template": "empty"})).unwrap();
    let source = app.run("graph.node.add", json!({"def": "uniform"})).unwrap()["id"].clone();
    app.run("graph.node.add", json!({"def": "output", "params": {"identifier": "mask"}, "connectFrom": {"node": source}})).unwrap();
    let directory = temp("batch");
    let mut settings = ExportSettings { destination: directory.display().to_string(), ..ExportSettings::default() };
    settings.batch = BatchExportSettings { width: 64, height: 32, linked: false };
    app.ui.export_tab = ExportTab::Batch;

    assert!(export(&mut app, &settings));

    assert_eq!(files(&directory), ["New_graph_base_color.png", "New_graph_height.png", "Second_mask.png"]);
    let image = materialcraft_engine::formats::decode(&std::fs::read(directory.join("Second_mask.png")).unwrap(), true).unwrap();
    assert_eq!((image.w, image.h), (64, 32));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn unwired_options_say_they_are_not_available_yet() {
    let mut app = app();
    let directory = temp("unwired");
    let planned = ExportSettings { destination: directory.display().to_string(), format: "dds".into(), ..ExportSettings::default() };

    for (tab, settings) in [(ExportTab::MaterialX, ExportSettings::default()), (ExportTab::Mcmat, ExportSettings::default()), (ExportTab::FromGraph, planned)] {
        app.ui.export_tab = tab;
        let toasts = app.toasts.len();

        assert!(!export(&mut app, &settings), "{tab:?} keeps the window open");

        let toast = app.toasts.get(toasts).map(|toast| toast.text.clone()).unwrap_or_default();
        assert!(toast.contains("isn't available yet"), "{tab:?}: {toast}");
    }
    assert!(!directory.exists(), "nothing is written");
}

#[test]
fn the_format_list_drops_tiff_and_lists_dds_as_planned() {
    let formats: Vec<ExportFormat> = ExportFormat::all().collect();

    assert!(formats.iter().all(|format| format.extension != "tif" && format.extension != "psd"));
    assert_eq!(ExportFormat::find("dds").map(|format| format.available), Some(false));
    assert_eq!(ExportFormat::find("png").map(ExportFormat::title).as_deref(), Some("PNG (*.png)"));
}
