#[cfg(test)]
mod unsaved_tests {
    use crate::unsaved::{Choice, Guarded, Step, answer};

    fn open() -> Guarded {
        Guarded::OpenPath { path: "a.sbs".into() }
    }

    #[test]
    fn cancel_drops_discard_runs_save_saves_first() {
        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Cancel), Step::Stop);
        assert_eq!(pending, None);

        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Discard), Step::Run(open()));
        assert_eq!(pending, None);

        let mut pending = Some(open());
        assert_eq!(answer(&mut pending, Choice::Save), Step::Save(open()));
        assert_eq!(pending, None);

        assert_eq!(answer(&mut None, Choice::Discard), Step::Stop, "nothing pending, nothing to run");
    }
}

#[test]
fn opening_packages_preserves_the_existing_unsaved_document() {
    let mut app = crate::MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
    app.session.modified = true;
    let bytes = |name: &str| materialcraft_engine::formats::sbs::write_sbs(&materialcraft_engine::core::Package::new(name)).unwrap();
    crate::web_files::load(&mut app, "first.sbs", bytes("First"), false);
    crate::web_files::load(&mut app, "second.sbs", bytes("Second"), false);
    assert!(app.ui.windows.unsaved.is_none());
    assert_eq!(app.session.open_packages().len(), 3);
    let result = crate::control::handle(&mut app, &egui::Context::default(), "ui.set", &serde_json::json!({"prefs": {"thumbnails": false}}));
    assert_eq!(result["ok"], true);
    app.run("package.activate", serde_json::json!({"document": 0})).unwrap();
    assert!(app.session.modified);
    assert_ne!(app.session.pkg.name, "First");
    assert_ne!(app.session.pkg.name, "Second");
}

#[test]
fn quitting_asks_about_each_modified_package() {
    use crate::unsaved::{Guarded, guard, perform};
    let mut app = crate::MaterialCraftApp::headless(materialcraft_engine::sample_session(), crate::UiState::default());
    app.session.modified = true;
    app.run("file.newPackage", serde_json::json!({"name": "Second"})).unwrap();
    app.session.modified = true;
    guard(&mut app, Guarded::Quit);
    assert_eq!(app.session.document_id, 0);
    app.ui.windows.unsaved = None;
    perform(&mut app, Guarded::Quit);
    assert!(!app.quit_confirmed);
    assert_eq!(app.session.pkg.name, "Second");
    assert_eq!(app.ui.windows.unsaved, Some(Guarded::Quit));
    app.ui.windows.unsaved = None;
    perform(&mut app, Guarded::Quit);
    assert!(app.quit_confirmed);
}

#[test]
fn a_pending_action_is_kept_even_if_the_package_was_saved_elsewhere() {
    let first = crate::unsaved::Guarded::Quit;
    let mut pending = Some(first.clone());
    assert!(crate::unsaved::request(&mut pending, false, crate::unsaved::Guarded::Menu { id: "ui.closeAll".into() }).is_none());
    assert_eq!(pending, Some(first));
}

#[test]
fn closing_one_package_only_prompts_for_that_packages_edits() {
    use crate::unsaved::{Guarded, guard, perform};
    let mut app = crate::MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
    app.session.modified = true;
    app.run("file.newPackage", serde_json::json!({"name": "Second"})).unwrap();
    let second = app.session.document_id;
    guard(&mut app, Guarded::Package { document: second, reload: false });
    assert!(app.ui.windows.unsaved.is_none());
    assert_eq!(app.session.document_id, 0);
    assert!(app.session.modified);

    app.run("file.newPackage", serde_json::json!({"name": "Third"})).unwrap();
    let third = app.session.document_id;
    guard(&mut app, Guarded::Package { document: 0, reload: false });
    assert_eq!(app.ui.windows.unsaved, Some(Guarded::Package { document: 0, reload: false }));
    app.ui.windows.unsaved = None;
    perform(&mut app, Guarded::Package { document: 0, reload: false });
    assert_eq!(app.session.document_id, third);
    assert_eq!(app.session.open_packages().len(), 1);
}
