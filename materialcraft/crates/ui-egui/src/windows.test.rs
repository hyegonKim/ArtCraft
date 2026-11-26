#[cfg(test)]
mod home_modality_tests {
    use egui_kittest::kittest::{By, NodeT, Queryable};

    use crate::{MaterialCraftApp, state::UiState};

    fn harness(block: impl FnOnce(&mut MaterialCraftApp)) -> egui_kittest::Harness<'static, MaterialCraftApp> {
        let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
        // The icon font has to be in place for the frame that first draws Home
        let mut fonts_installed = false;
        let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
            move |ui, app: &mut MaterialCraftApp| {
                if std::mem::replace(&mut fonts_installed, true) {
                    crate::windows::show(app, ui.ctx());
                } else {
                    crate::theme::install(ui.ctx());
                }
            },
            app,
        );
        let app = harness.state_mut();
        app.ui.windows.home = true;
        app.ui.recent.push("recent.sbs".into());
        block(app);
        harness.run_steps(3);
        harness
    }

    fn file_dialog(app: &mut MaterialCraftApp) {
        app.dialogs.ask(craft_dialogs::FileDialog::open(), crate::native_dialogs::Then::OpenPackage);
    }

    fn open_package(harness: &egui_kittest::Harness<'static, MaterialCraftApp>) -> bool {
        !harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Open package")).accesskit_node().is_disabled()
    }

    #[test]
    fn home_actions_are_live_when_nothing_blocks_them() {
        let harness = harness(|_| {});
        assert!(open_package(&harness));
        assert!(!harness.get(By::new().role(egui::accesskit::Role::Button).label("New graph")).accesskit_node().is_disabled());
    }

    #[test]
    fn open_package_is_off_behind_a_dialog_and_home_stays_open() {
        let harness = harness(|app| app.ui.windows.about = true);
        assert!(!open_package(&harness));
        assert!(harness.get(By::new().role(egui::accesskit::Role::Button).label("New graph")).accesskit_node().is_disabled());
        assert!(harness.state().ui.windows.home);
    }

    #[test]
    fn open_package_brings_a_file_dialog_forward() {
        let mut harness = harness(file_dialog);
        assert!(open_package(&harness));
        harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Open package")).click();
        harness.run_steps(3);
        assert!(!harness.state().ui.windows.home);
        assert_eq!(harness.state().dialogs.queued(), 1, "no second dialog is asked for");
    }

    #[test]
    fn a_sample_tile_and_a_recent_link_wait_behind_a_file_dialog() {
        let mut harness = harness(file_dialog);
        assert!(harness.get(By::new().role(egui::accesskit::Role::Link).label("recent.sbs")).accesskit_node().is_disabled());
        let graphs = harness.state().session.pkg.graphs.len();
        let (_, sample) = materialcraft_engine::samples::TEMPLATES.iter().find(|(id, _)| !matches!(*id, "empty" | "pbr")).unwrap();
        harness.get(By::new().role(egui::accesskit::Role::ListItem).label(sample)).click();
        harness.run_steps(3);
        assert_eq!(harness.state().session.pkg.graphs.len(), graphs);
        assert!(harness.state().ui.windows.home);
    }

    #[test]
    fn sample_tiles_are_disabled_only_behind_a_dialog() {
        let (_, sample) = materialcraft_engine::samples::TEMPLATES.iter().find(|(id, _)| !matches!(*id, "empty" | "pbr")).unwrap();
        let tile = By::new().role(egui::accesskit::Role::ListItem).label(sample);
        assert!(!harness(|_| {}).get(tile).accesskit_node().is_disabled());
        let tile = By::new().role(egui::accesskit::Role::ListItem).label(sample);
        assert!(harness(|app| app.ui.windows.about = true).get(tile).accesskit_node().is_disabled());
    }
}

#[cfg(test)]
mod open_guard_tests {
    use crate::{MaterialCraftApp, state::UiState};

    #[test]
    fn a_failed_open_keeps_unsaved_work_without_a_discard_prompt() {
        let mut s = materialcraft_engine::sample_session();
        s.modified = true;
        let mut app = MaterialCraftApp::headless(s, UiState::default());
        let before = app.session.pkg.clone();
        crate::windows::open_path(&mut app, "does-not-exist.sbs");
        assert!(app.ui.windows.unsaved.is_none());
        assert!(!app.toasts.is_empty());
        assert_eq!(app.session.pkg, before);
        assert!(app.session.modified);
    }

    #[test]
    fn opening_a_path_over_a_clean_package_goes_straight_to_open() {
        let mut s = materialcraft_engine::sample_session();
        s.modified = false;
        let mut app = MaterialCraftApp::headless(s, UiState::default());
        crate::windows::open_path(&mut app, "does-not-exist.sbs");
        assert!(app.ui.windows.unsaved.is_none());
        assert!(!app.toasts.is_empty(), "the open was attempted and its error reported");
    }
}

#[cfg(test)]
mod import_tests {
    use crate::{MaterialCraftApp, state::UiState};
    use materialcraft_engine::formats::{Depth, encode};
    use materialcraft_engine::image::{Image, Kind};

    #[test]
    fn importing_a_bitmap_file_is_one_undo_entry() {
        let png = encode(&Image::filled(4, 4, Kind::Color, [0.5, 0.2, 0.1, 1.0]), "png", Depth::U8, true).unwrap();
        let path = std::env::temp_dir().join(format!("mc-import-{}.png", std::process::id()));
        std::fs::write(&path, png).unwrap();
        let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
        let base = app.session.history().len();
        crate::graph::import_bitmap_file(&mut app, &path.display().to_string());
        std::fs::remove_file(&path).ok();
        assert_eq!(app.session.history().len(), base + 1, "history: {:?}", app.session.history());
        assert_eq!(app.session.history().last().map(String::as_str), Some("Import bitmap"));
        app.session.undo();
        assert_eq!(app.session.history().len(), base);
    }
}
#[test]
fn project_dialogs_offer_separate_formats_and_import_filters_show_full_names() {
    use super::{package_filters, resource_filters};
    let saved = package_filters(false, false);

    assert_eq!(saved, vec![("SBS XML Package (*.sbs)", &["sbs"][..]), ("MaterialCraft Project (*.mcraft)", &["mcraft"][..])]);
    assert_eq!(package_filters(false, true)[0], saved[1]);

    let opened = package_filters(true, false);

    assert_eq!(opened[0], ("All formats (*.sbs *.mcraft)", &["sbs", "mcraft"][..]));
    assert_eq!(&opened[1..], &saved);

    let bitmap = resource_filters(false);

    assert_eq!(bitmap[0].1, materialcraft_engine::formats::BITMAP_EXTENSIONS);
    assert!(bitmap.iter().any(|(label, _)| label == "Portable Network Graphics (*.png)"));
    assert!(bitmap.iter().any(|(label, _)| label == "JPEG - JFIF Compliant (*.jpg *.jif *.jpeg *.jpe)"));
    assert_eq!(resource_filters(true)[1].0, "Scalable Vector Graphics (*.svg)");
}
