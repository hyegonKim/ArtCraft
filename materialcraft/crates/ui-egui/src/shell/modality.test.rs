use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Blocker, Modality};
use crate::explorer::ExplorerTarget;
use crate::{MaterialCraftApp, UiState};

fn app() -> MaterialCraftApp {
    MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default())
}

fn capture(app: &MaterialCraftApp) -> Modality {
    Modality::capture(app, &egui::Context::default())
}

#[test]
fn home_and_the_shortcuts_list_leave_input_free() {
    let mut app = app();
    assert_eq!(capture(&app), Modality::Free);
    app.ui.windows.home = true;
    app.ui.windows.shortcuts = true;
    assert_eq!(capture(&app), Modality::Free);
}

#[test]
fn every_blocking_source_blocks() {
    type Open = fn(&mut MaterialCraftApp);
    let cases: [(&str, Open, Blocker); 10] = [
        ("new graph", |app| app.ui.windows.new_graph = true, Blocker::Dialog),
        ("export", |app| app.ui.windows.export = true, Blocker::Dialog),
        ("preferences", |app| app.ui.windows.preferences = true, Blocker::Dialog),
        ("about", |app| app.ui.windows.about = true, Blocker::Dialog),
        ("rename", |app| app.ui.explorer.rename = Some((ExplorerTarget::Folder { document: 0, path: "a".into() }, "a".into())), Blocker::Dialog),
        ("web save", |app| crate::web_files::PackageSave::request(app, false), Blocker::Dialog),
        ("unsaved prompt", |app| app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit), Blocker::UnsavedPrompt),
        ("awaited save", |app| app.awaiting_save = Some(crate::unsaved::Guarded::Quit), Blocker::UnsavedPrompt),
        (
            "queued file dialog",
            |app| app.dialogs.ask(craft_dialogs::FileDialog::save(), crate::native_dialogs::Then::ExportDestination(crate::export::ExportTab::FromGraph)),
            Blocker::FileDialog,
        ),
        ("update prompt", |app| crate::updates_ui::open_prompt_for_test(app), Blocker::Update),
    ];

    let wrong: Vec<_> = cases
        .into_iter()
        .filter_map(|(name, open, blocker)| {
            let mut app = app();
            open(&mut app);
            (capture(&app) != Modality::Blocked(blocker)).then_some(name)
        })
        .collect();
    assert!(wrong.is_empty(), "not reported as blocking: {wrong:?}");
}

#[test]
fn a_blocker_lets_only_its_own_commands_through() {
    assert!(Modality::Free.allows("edit.delete"));
    for id in ["edit.delete", "file.open", "file.save"] {
        assert!(!Modality::Blocked(Blocker::UnsavedPrompt).allows(id), "{id}");
        assert!(!Modality::Blocked(Blocker::Dialog).allows(id), "{id}");
        assert!(!Modality::Blocked(Blocker::Modal).allows(id), "{id}");
        assert!(!Modality::Blocked(Blocker::Update).allows(id), "{id}");
    }

    let file_dialog = Modality::Blocked(Blocker::FileDialog);
    for id in ["file.open", "file.saveAs", "file.saveCopyAs", "file.importBitmap", "file.importVector"] {
        assert!(file_dialog.allows(id), "{id}");
    }
    assert!(!file_dialog.allows("file.exportMaterial"), "Export material opens the Export window, which waits for the file dialog");
    assert!(!file_dialog.allows("file.save"), "File > Save writes without showing a dialog when the package has a path");
    assert!(!file_dialog.allows("edit.delete"));
}

#[test]
fn a_file_dialog_frees_its_commands_only_when_nothing_else_blocks() {
    let queued = |app: &mut MaterialCraftApp| app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage);

    let mut preferences = app();
    queued(&mut preferences);
    preferences.ui.windows.preferences = true;
    assert_eq!(capture(&preferences), Modality::Blocked(Blocker::Dialog));
    assert!(!capture(&preferences).allows("file.open"));

    let mut update = app();
    queued(&mut update);
    crate::updates_ui::open_prompt_for_test(&mut update);
    assert_eq!(capture(&update), Modality::Blocked(Blocker::Update));

    let mut modal = app();
    queued(&mut modal);
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::Modal::new(egui::Id::unique("modal")).show(ui.ctx(), |_| {});
    })
    .textures_delta
    .clear();
    let mut answer = Modality::Free;
    ctx.run_ui(egui::RawInput::default(), |ui| answer = Modality::capture(&modal, ui.ctx())).textures_delta.clear();
    assert_eq!(answer, Modality::Blocked(Blocker::Modal));
}

#[test]
fn a_modal_layer_from_the_previous_frame_blocks() {
    let app = app();
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::Modal::new(egui::Id::unique("modal")).show(ui.ctx(), |_| {});
    })
    .textures_delta
    .clear();
    let mut answer = Modality::Free;
    ctx.run_ui(egui::RawInput::default(), |ui| answer = Modality::capture(&app, ui.ctx())).textures_delta.clear();
    assert_eq!(answer, Modality::Blocked(Blocker::Modal));
}

#[derive(Debug)]
struct Dropped(PathBuf, Vec<u8>);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self.1.clone())
    }
}

/// One whole frame with `files` dropped on the window
fn frame(app: &mut MaterialCraftApp, ctx: &egui::Context, files: Vec<egui::DroppedFileHandle>) {
    let input = egui::RawInput { dropped_files: files, ..Default::default() };
    ctx.run_ui(input, |ui| {
        app.step(ui.ctx());
        app.draw(ui);
    })
    .textures_delta
    .clear();
}

#[test]
fn a_file_dropped_behind_a_prompt_does_not_reach_the_package() {
    use materialcraft_engine::formats::{Depth, encode};
    use materialcraft_engine::image::{Image, Kind};

    let mut app = app();
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, Vec::new());
    frame(&mut app, &ctx, Vec::new());
    let png = encode(&Image::filled(2, 2, Kind::Gray, [0.5; 4]), "png", Depth::U8, false).unwrap();
    let dropped = || -> Vec<egui::DroppedFileHandle> { vec![Arc::new(Dropped("dropped.png".into(), png.clone()))] };
    let nodes = app.session.graph().unwrap().nodes.len();

    app.ui.windows.unsaved = Some(crate::unsaved::Guarded::Quit);
    frame(&mut app, &ctx, dropped());
    assert_eq!(app.session.graph().unwrap().nodes.len(), nodes);

    app.ui.windows.unsaved = None;
    frame(&mut app, &ctx, dropped());
    assert_eq!(app.session.graph().unwrap().nodes.len(), nodes + 1, "the drop works once the prompt is answered");
}
