use super::*;
use crate::UiState;
use crate::unsaved::Guarded;
use craft_dialogs::Request;

fn app() -> MaterialCraftApp {
    MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default())
}

/// Answer every queued dialog with `answer` and collect what was asked
fn answer_all(app: &mut MaterialCraftApp, answer: impl Fn() -> Answer + 'static) -> Vec<Request> {
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = asked.clone();
    app.dialogs.set_override(Some(Box::new(move |request: &Request| {
        log.borrow_mut().push(request.clone());
        answer()
    })));
    app.dialogs.show_pending(None::<&eframe::Frame>);
    poll(app);
    asked.take()
}

fn temp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mc-dialogs-{}-{name}", std::process::id()))
}

#[test]
fn package_dialogs_keep_both_project_formats() {
    for (command, open) in [("file.open", true), ("file.saveAs", false), ("file.saveCopyAs", false)] {
        let mut app = app();

        crate::windows::file_action(&mut app, &egui::Context::default(), command);
        let asked = answer_all(&mut app, || Answer::Cancelled);

        let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
        let expected: Vec<_> =
            crate::windows::package_filters(open, false).into_iter().map(|(name, extensions)| craft_dialogs::Filter::new(name, extensions)).collect();
        assert_eq!(dialog.filters, expected, "{command}");
    }
}

#[test]
fn archive_save_dialog_keeps_the_mcraft_default() {
    for command in ["file.saveAs", "file.saveCopyAs"] {
        let mut app = app();
        app.session.path = Some("project.mcraft".into());

        crate::windows::file_action(&mut app, &egui::Context::default(), command);
        let asked = answer_all(&mut app, || Answer::Cancelled);

        let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
        assert_eq!(dialog.file_name.as_deref(), Some(format!("{}.mcraft", app.session.pkg.name).as_str()));
        assert_eq!(dialog.filters.first(), Some(&craft_dialogs::Filter::new("MaterialCraft Project (*.mcraft)", &["mcraft"])));
    }
}

#[test]
fn bitmap_dialog_keeps_the_supported_format_filters() {
    let mut app = app();

    crate::windows::file_action(&mut app, &egui::Context::default(), "file.importBitmap");
    let asked = answer_all(&mut app, || Answer::Cancelled);

    let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
    let expected: Vec<_> = crate::windows::resource_filters(false).into_iter().map(|(name, extensions)| craft_dialogs::Filter::new(name, extensions)).collect();
    assert_eq!(dialog.filters, expected);
}

#[test]
fn a_picked_archive_saves_and_reopens_with_resources() {
    for copy in [false, true] {
        let mut app = app();
        let image = png(&format!("archive-{copy}.png"));
        app.run("file.importBitmap", json!({"path": image})).unwrap();
        app.session.path = Some("original.mcraft".into());
        let original = app.session.pkg.clone();
        let path = temp(&format!("archive-{copy}.mcraft"));
        let target = path.clone();

        crate::windows::file_action(&mut app, &egui::Context::default(), if copy { "file.saveCopyAs" } else { "file.saveAs" });
        answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));

        assert!(path.exists());
        assert_eq!(app.session.modified, copy);
        if copy {
            assert_eq!(app.session.path.as_deref(), Some("original.mcraft"));
            assert!(app.ui.recent.is_empty());
        }

        crate::windows::file_action(&mut app, &egui::Context::default(), "file.open");
        let target = path.clone();
        answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));

        assert_eq!(app.session.path.as_deref().map(std::path::PathBuf::from), std::fs::canonicalize(&path).ok());
        assert_eq!(app.session.pkg.resources.len(), original.resources.len());
        assert_eq!(app.session.pkg.resources.last().unwrap().bytes, original.resources.last().unwrap().bytes);
        assert!(app.toasts.iter().all(|toast| !toast.error));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(image).unwrap();
    }
}

#[test]
fn resource_dialogs_keep_the_originating_package_and_folder() {
    for vector in [false, true] {
        for closed in [false, true] {
            let mut app = app();
            let document = app.session.document_id;
            app.run("folder.create", json!({"parent": "Resources", "name": "Imported"})).unwrap();
            let nodes = app.session.graph().unwrap().nodes.len();
            let path = if vector {
                let path = temp(&format!("resource-{closed}.svg"));
                std::fs::write(&path, r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><path d="M0 0H4V4H0Z"/></svg>"#).unwrap();
                path
            } else {
                png(&format!("resource-{closed}.png"))
            };

            crate::windows::import_resource_into(&mut app, &egui::Context::default(), vector, "Resources/Imported");
            assert_eq!(app.dialogs.queued(), 1);
            app.run("file.newPackage", json!({})).unwrap();
            let other = app.session.document_id;
            if closed {
                app.run("package.close", json!({"document": document, "discard": true})).unwrap();
            }
            let target = path.clone();
            let asked = answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));

            let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
            let expected: Vec<_> =
                crate::windows::resource_filters(vector).into_iter().map(|(name, extensions)| craft_dialogs::Filter::new(name, extensions)).collect();
            assert_eq!(dialog.filters, expected);
            assert!(app.session.package(other).unwrap().resources.is_empty());
            if closed {
                assert_eq!(app.session.document_id, other);
                assert!(app.toasts.iter().any(|toast| toast.error));
            } else {
                assert_eq!(app.session.document_id, document);
                assert_eq!(app.session.graph().unwrap().nodes.len(), nodes);
                let resource = app.session.pkg.resources.last().unwrap();
                assert_eq!(resource.folder, "Resources/Imported");
                assert_eq!(resource.bytes.as_slice(), std::fs::read(&path).unwrap());
                app.run("edit.undo", json!({})).unwrap();
                assert!(app.session.pkg.resources.is_empty());
            }
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn save_as_writes_the_package_and_adds_it_to_recent() {
    let mut app = app();
    let path = temp("saved.sbs");
    assert!(crate::windows::file_action(&mut app, &egui::Context::default(), "file.saveAs"));
    let target = path.clone();
    let asked = answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
    assert!(matches!(dialog.kind, craft_dialogs::Kind::Save));
    assert_eq!(dialog.file_name.as_deref(), Some(format!("{}.sbs", app.session.pkg.name).as_str()));
    assert!(path.exists());
    assert!(!app.ui.recent.is_empty());
    assert_eq!(app.session.path.as_deref().map(std::path::PathBuf::from), std::fs::canonicalize(&path).ok());
    std::fs::remove_file(path).ok();
}

#[test]
fn a_canceled_dialog_changes_nothing() {
    let mut app = app();
    let before = app.session.pkg.clone();
    crate::windows::file_action(&mut app, &egui::Context::default(), "file.saveAs");
    let asked = answer_all(&mut app, || Answer::Cancelled);
    assert_eq!(asked.len(), 1);
    assert_eq!(app.session.pkg, before);
    assert!(app.toasts.is_empty());
}

#[test]
fn an_unavailable_dialog_reports_why() {
    let mut app = app();
    crate::windows::file_action(&mut app, &egui::Context::default(), "file.saveAs");
    answer_all(&mut app, || Answer::Unavailable("no display".into()));
    assert!(app.toasts.iter().any(|toast| toast.error && toast.text.contains("no display")), "{:?}", app.toasts.len());
}

#[test]
fn a_second_request_while_a_dialog_is_waiting_is_not_queued() {
    let mut app = app();
    let ctx = egui::Context::default();
    crate::windows::file_action(&mut app, &ctx, "file.open");
    crate::windows::file_action(&mut app, &ctx, "file.open");
    assert_eq!(app.dialogs.queued(), 1);
}

#[test]
fn save_goes_straight_to_a_path_the_package_has() {
    let mut app = app();
    let path = temp("direct.sbs");
    app.session.path = Some(path.display().to_string());
    crate::windows::file_action(&mut app, &egui::Context::default(), "file.save");
    assert_eq!(app.dialogs.queued(), 0, "no dialog for a package with a path");
    assert!(path.exists());
    std::fs::remove_file(path).ok();
}

fn modified_app_asking_to_quit() -> MaterialCraftApp {
    let mut app = app();
    app.session.modified = true;
    app.session.path = None;
    crate::unsaved::guard(&mut app, Guarded::Quit);
    assert_eq!(app.ui.windows.unsaved, Some(Guarded::Quit));
    app
}

fn answer_prompt_with_save(app: &mut MaterialCraftApp) {
    let step = crate::unsaved::answer(&mut app.ui.windows.unsaved, crate::unsaved::Choice::Save);
    let crate::unsaved::Step::Save(action) = step else { panic!("{step:?}") };
    crate::unsaved::save_then(app, &egui::Context::default(), action);
}

#[test]
fn the_prompts_save_with_a_path_saves_and_runs_the_action_at_once() {
    let mut app = modified_app_asking_to_quit();
    let path = temp("prompt-direct.sbs");
    app.session.path = Some(path.display().to_string());
    answer_prompt_with_save(&mut app);
    assert_eq!(app.dialogs.queued(), 0, "no dialog for a package with a path");
    assert!(path.exists());
    assert!(app.quit_confirmed);
    assert!(app.awaiting_save.is_none());
    std::fs::remove_file(path).ok();
}

#[test]
fn the_app_shows_a_requested_dialog_and_applies_its_answer() {
    let mut harness = egui_kittest::Harness::builder().build_eframe(|_| app());
    harness.step();
    harness.step();
    let path = temp("frames.sbs");
    let target = path.clone();
    let ctx = harness.ctx.clone();
    let state = harness.state_mut();
    state.dialogs.set_override(Some(Box::new(move |_: &Request| Answer::Paths(vec![target.clone()]))));
    crate::windows::file_action(state, &ctx, "file.saveAs");
    assert_eq!(state.dialogs.queued(), 1);
    harness.step();
    assert_eq!(harness.state().dialogs.queued(), 0, "the frame shows the dialog");
    assert!(!path.exists(), "the answer is applied on the next frame");
    harness.step();
    assert!(path.exists(), "the frame applies the answer");
    std::fs::remove_file(path).ok();
}

#[test]
fn a_picked_package_opens() {
    let mut first = app();
    let path = temp("opened.sbs");
    let document = first.session.document_id;
    assert!(save_to(&mut first, document, false, &path.display().to_string()));

    let mut app = app();
    crate::windows::file_action(&mut app, &egui::Context::default(), "file.open");
    let target = path.clone();
    answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    assert_eq!(app.session.path.as_deref().map(std::path::PathBuf::from), std::fs::canonicalize(&path).ok());
    assert!(app.toasts.iter().any(|toast| !toast.error && toast.text.starts_with("Opened")), "{}", app.toasts.len());
    std::fs::remove_file(path).ok();
}

#[test]
fn export_material_opens_the_export_window_on_the_mcmat_tab() {
    let mut app = app();

    crate::windows::file_action(&mut app, &egui::Context::default(), "file.exportMaterial");

    assert!(app.ui.windows.export);
    assert_eq!(app.ui.export_tab, crate::export::ExportTab::Mcmat);
    assert!(!app.dialogs.is_open() && app.dialogs.queued() == 0, "the window picks the destination, not a dialog of its own");
}

fn png(name: &str) -> std::path::PathBuf {
    use materialcraft_engine::formats::{Depth, encode};
    let image = Image::filled(4, 4, materialcraft_engine::image::Kind::Gray, [0.7; 4]);
    let path = temp(name);
    std::fs::write(&path, encode(&image, "png", Depth::U16, false).unwrap()).unwrap();
    path
}

#[test]
fn an_imported_bitmap_joins_the_active_package() {
    let mut app = app();
    let before = app.session.pkg.resources.len();
    crate::windows::file_action(&mut app, &egui::Context::default(), "file.importBitmap");
    let path = png("imported.png");
    let target = path.clone();
    answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    assert_eq!(app.session.pkg.resources.len(), before + 1);
    std::fs::remove_file(path).ok();
}

#[test]
fn a_picked_mesh_shows_in_the_3d_view() {
    let mut app = app();
    crate::web_files::load_mesh(&mut app, &egui::Context::default());
    let path = temp("mesh.obj");
    std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    let target = path.clone();
    answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    assert!(app.session.custom_mesh.is_some());
    assert_eq!(app.session.view3d.scene, "custom");
    std::fs::remove_file(path).ok();
}

#[test]
fn the_2d_views_image_is_written_to_the_picked_file() {
    for ext in ["png", "exr"] {
        let mut app = app();
        app.session.cook();
        crate::view2d::save_image(&mut app);
        let path = temp(&format!("view.{ext}"));
        let target = path.clone();
        let asked = answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
        let Some(Request::File(dialog)) = asked.first() else { panic!("{asked:?}") };
        assert!(dialog.file_name.as_deref().is_some_and(|name| name.ends_with(".png")));
        assert!(materialcraft_engine::formats::decode(&std::fs::read(&path).unwrap(), false).is_ok(), "{ext}");
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn a_picked_input_image_is_bound_on_the_active_graph() {
    let mut app = app();
    app.run("graph.node.add", json!({"def": "input_grayscale", "params": {"identifier": "curvature"}}));
    let path = png("input.png");
    app.dialogs.ask(FileDialog::open(), Then::BindInput(json!({"identifier": "curvature"}), GraphRef::active(&app)));
    let target = path.clone();
    answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    assert!(app.session.graph().unwrap().input_images.contains_key("curvature"));
    std::fs::remove_file(path).ok();
}

#[test]
fn the_prompts_save_waits_for_its_dialog_and_then_quits() {
    let mut app = modified_app_asking_to_quit();
    answer_prompt_with_save(&mut app);

    // While the dialog is up, another close asks nothing and the app is not quitting
    assert!(!app.quit_confirmed);
    crate::unsaved::guard(&mut app, Guarded::Quit);
    assert_eq!(app.ui.windows.unsaved, None, "no second prompt while the Save dialog is open");

    let path = temp("quit.sbs");
    let target = path.clone();
    answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    assert!(path.exists());
    assert!(!app.session.modified);
    assert!(app.quit_confirmed, "the held quit runs once the package is saved");
    assert!(app.awaiting_save.is_none());
    std::fs::remove_file(path).ok();
}

#[test]
fn canceling_the_prompts_save_dialog_keeps_the_app_open() {
    let mut app = modified_app_asking_to_quit();
    app.pending_load = Some(("waiting.sbs".into(), vec![1, 2, 3]));
    answer_prompt_with_save(&mut app);
    answer_all(&mut app, || Answer::Cancelled);
    assert!(!app.quit_confirmed);
    assert!(app.awaiting_save.is_none());
    assert!(app.pending_load.is_none(), "the held load is dropped");
    assert!(app.session.modified);
}

#[test]
fn a_held_close_waits_for_the_save_dialog() {
    let mut app = modified_app_asking_to_quit();
    answer_prompt_with_save(&mut app);
    app.session.modified = false;
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    let mut output = ctx.run_ui(input, |ui| crate::unsaved::handle_close(&mut app, ui.ctx()));
    output.textures_delta.clear();
    let commands = output.viewport_output.remove(&egui::ViewportId::ROOT).map(|root| root.commands).unwrap_or_default();
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
}

#[test]
fn a_held_close_restores_a_minimized_window() {
    for (minimized, restored) in [(Some(true), true), (Some(false), false), (None, false)] {
        let mut app = modified_app_asking_to_quit();
        app.ui.windows.unsaved = None;
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        let root = input.viewports.entry(egui::ViewportId::ROOT).or_default();
        root.minimized = minimized;
        root.events.push(egui::ViewportEvent::Close);
        let mut output = ctx.run_ui(input, |ui| crate::unsaved::handle_close(&mut app, ui.ctx()));
        output.textures_delta.clear();
        let commands = output.viewport_output.remove(&egui::ViewportId::ROOT).map(|root| root.commands).unwrap_or_default();
        assert_eq!(commands.contains(&egui::ViewportCommand::Minimized(false)), restored, "{minimized:?}");
    }
}

#[test]
fn a_saved_package_goes_to_the_document_that_asked() {
    let mut app = app();
    let first = app.session.document_id;
    app.run("file.newPackage", serde_json::json!({}));
    let second = app.session.document_id;
    assert_ne!(first, second, "a second package is open");

    let path = temp("first.sbs");
    let saved = save_to(&mut app, first, false, &path.display().to_string());
    assert!(saved);
    assert_eq!(app.session.document_id, first, "the asking package is active again");
    assert_eq!(app.session.path.as_deref().map(std::path::PathBuf::from), std::fs::canonicalize(&path).ok());
    std::fs::remove_file(path).ok();
}

#[test]
fn saving_a_package_that_closed_meanwhile_does_not_write() {
    let mut app = app();
    let path = temp("gone.sbs");
    assert!(!save_to(&mut app, u64::MAX, false, &path.display().to_string()));
    assert!(!path.exists());
    assert!(!app.toasts.is_empty(), "the error is reported");
}

#[test]
fn export_destination_dialogs_fill_their_tab_field() {
    use crate::export::ExportTab;

    let mut app = app();
    for (tab, name) in [(ExportTab::FromGraph, "images"), (ExportTab::MaterialX, "materialx"), (ExportTab::Mcmat, "material.mcmat")] {
        app.dialogs.ask(FileDialog::folder(), Then::ExportDestination(tab));
        let target = std::env::temp_dir().join(name);
        answer_all(&mut app, move || Answer::Paths(vec![target.clone()]));
    }

    let draft = app.ui.export_draft.as_ref().unwrap();
    let picked = |name: &str| std::env::temp_dir().join(name).display().to_string();
    assert_eq!(draft.destination, picked("images"));
    assert_eq!(draft.material.folder, picked("materialx"));
    assert_eq!(draft.material.archive, picked("material.mcmat"));
    assert_eq!(app.ui.export, crate::export::ExportSettings::default(), "picking edits the window's copy until it is saved");
}

#[test]
fn a_picked_input_image_is_not_applied_to_another_graph() {
    let mut app = app();
    let target = GraphRef { path: "another graph".into(), ..GraphRef::active(&app) };
    app.dialogs.ask(FileDialog::open(), Then::BindInput(json!({"identifier": "curvature"}), target));
    let before = app.session.history().len();
    answer_all(&mut app, || Answer::Paths(vec!["image.png".into()]));
    assert_eq!(app.session.history().len(), before);
    assert!(app.toasts.iter().any(|toast| toast.error && toast.text.starts_with("Not applied")), "{}", app.toasts.len());
}

#[test]
fn an_imported_bitmap_is_not_added_to_another_graph() {
    let mut app = app();
    let target = GraphRef { path: "another graph".into(), ..GraphRef::active(&app) };
    app.dialogs.ask(FileDialog::open(), Then::ImportBitmap(target));
    let before = app.session.history().len();
    answer_all(&mut app, || Answer::Paths(vec!["does-not-exist.png".into()]));
    assert_eq!(app.session.history().len(), before);
    assert!(app.toasts.iter().any(|toast| toast.text.starts_with("Not applied")), "{}", app.toasts.len());
}

#[test]
fn a_graph_that_took_over_the_index_is_a_different_graph() {
    let mut app = app();
    app.run("graph.duplicate", json!({}));
    app.run("graph.open", json!({"graph": 0}));
    let target = GraphRef::active(&app);
    app.run("graph.delete", json!({"graph": 0}));
    assert_eq!(app.session.graph, 0, "another graph holds the index now");
    assert!(GraphRef::active(&app) != target);
}

#[test]
fn a_pick_for_a_renamed_graph_says_the_graph_changed() {
    let mut app = app();
    let target = GraphRef::active(&app);
    app.session.graph_mut().unwrap().name = "renamed".into();
    app.dialogs.ask(FileDialog::open(), Then::ImportBitmap(target));
    answer_all(&mut app, || Answer::Paths(vec!["does-not-exist.png".into()]));
    assert!(app.toasts.iter().any(|toast| toast.text.contains("graph or package changed")), "{}", app.toasts.len());
}

#[test]
fn a_graph_of_the_same_name_in_another_folder_is_a_different_graph() {
    let mut app = app();
    app.session.graph_mut().unwrap().folder = "first".into();
    let target = GraphRef::active(&app);
    app.session.graph_mut().unwrap().folder = "second".into();
    assert!(GraphRef::active(&app) != target);
}

#[test]
fn a_mesh_picked_for_another_package_is_not_loaded() {
    let mut app = app();
    let document = app.session.document_id + 1;
    app.dialogs.ask(FileDialog::open(), Then::LoadMesh { document });
    answer_all(&mut app, || Answer::Paths(vec!["does-not-exist.obj".into()]));
    assert!(app.toasts.iter().any(|toast| toast.text.starts_with("Not applied")), "{}", app.toasts.len());
}
