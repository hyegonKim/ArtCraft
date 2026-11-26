use super::*;
use crate::unsaved::{self, Choice, Step};
use egui_kittest::kittest::Queryable as _;
use serde_json::json;

fn app() -> MaterialCraftApp {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
    app.session.modified = true;
    app
}

fn choose_save(app: &mut MaterialCraftApp, action: Guarded) {
    unsaved::guard(app, action);
    let Step::Save(action) = unsaved::answer(&mut app.ui.windows.unsaved, Choice::Save) else { panic!("expected the unsaved prompt") };
    PackageSave::save_then(app, action, |_, _| panic!("an unnamed package must wait for the Save dialog"));
}

#[test]
fn browser_save_before_close_waits_for_download() {
    for close_all in [false, true] {
        let mut app = app();
        let document = app.session.document_id;
        let action = if close_all { Guarded::Menu { id: "ui.closeAll".into() } } else { Guarded::Package { document, reload: false } };

        choose_save(&mut app, action.clone());

        assert!(app.session.has_package());
        assert!(app.session.modified);
        assert_eq!(app.ui.windows.package_save.as_ref().unwrap().document, document);
        assert_eq!(app.awaiting_save, Some(action.clone()));

        PackageSave::finish(&mut app, true, |name, bytes| {
            assert!(name.ends_with("_sbs.zip"));
            assert!(zip::ZipArchive::new(std::io::Cursor::new(bytes)).is_ok());
            Ok(())
        });

        assert!(!app.session.has_package(), "{action:?} must resume after Download");
        assert!(app.awaiting_save.is_none());
        assert!(app.ui.windows.package_save.is_none());
    }
}

#[test]
fn browser_save_before_close_keeps_its_document_and_cannot_be_replaced_by_save_copy() {
    let mut app = app();
    let document = app.session.document_id;
    choose_save(&mut app, Guarded::Package { document, reload: false });
    let request = app.ui.windows.package_save.clone();
    app.run("file.newPackage", json!({"name": "Other"})).unwrap();
    let other = app.session.document_id;
    app.session.modified = true;

    PackageSave::request(&mut app, true);

    assert_eq!(app.ui.windows.package_save, request);

    PackageSave::finish(&mut app, true, |_, _| Ok(()));

    assert!(app.session.package(document).is_err());
    assert_eq!(app.session.document_id, other);
    assert!(app.session.modified);
}

#[test]
fn browser_cancel_or_download_error_releases_the_action_and_pending_load() {
    for cancel in [false, true] {
        let mut app = app();
        choose_save(&mut app, Guarded::Quit);
        app.pending_load = Some(("pending.sbs".into(), vec![]));

        PackageSave::finish(&mut app, !cancel, |_, _| Err("download failed".into()));

        assert!(app.session.has_package());
        assert!(app.session.modified);
        assert!(app.session.path.is_none());
        assert!(!app.quit_confirmed);
        assert!(app.awaiting_save.is_none());
        assert!(app.pending_load.is_none());
        assert!(app.ui.windows.package_save.is_none());
        if !cancel {
            assert!(app.toasts.iter().any(|toast| toast.error && toast.text.contains("download failed")));
        }

        PackageSave::save(&mut app, |_, _| panic!("the package still needs a name"));
        PackageSave::finish(&mut app, true, |_, _| Ok(()));

        assert!(!app.session.modified);
        assert!(!app.quit_confirmed, "a later save must not resume the abandoned quit");
    }
}

#[test]
fn browser_download_for_a_closed_document_cancels_the_held_action() {
    let mut app = app();
    choose_save(&mut app, Guarded::Menu { id: "ui.closeAll".into() });
    app.run("package.close", json!({"discard": true})).unwrap();
    app.run("file.newPackage", json!({"name": "Other"})).unwrap();
    app.session.modified = true;

    PackageSave::finish(&mut app, true, |_, _| panic!("a closed package must not download"));

    assert_eq!(app.session.pkg.name, "Other");
    assert!(app.session.modified);
    assert!(app.session.path.is_none());
    assert!(app.awaiting_save.is_none());
    assert!(app.ui.windows.unsaved.is_none());
    assert!(app.ui.windows.package_save.is_none());
}

#[test]
fn browser_close_all_and_quit_wait_for_each_modified_package() {
    for action in [Guarded::Menu { id: "ui.closeAll".into() }, Guarded::Quit] {
        let mut app = app();
        let first = app.session.document_id;
        app.run("file.newPackage", json!({"name": "Second"})).unwrap();
        let second = app.session.document_id;
        app.session.modified = true;

        choose_save(&mut app, action.clone());
        assert_eq!(app.ui.windows.package_save.as_ref().unwrap().document, first);
        PackageSave::finish(&mut app, true, |_, _| Ok(()));

        assert!(!app.quit_confirmed);
        assert_eq!(app.ui.windows.unsaved, Some(action.clone()));
        assert_eq!(app.session.document_id, second);
        assert!(app.session.modified);

        choose_save(&mut app, action.clone());
        PackageSave::finish(&mut app, true, |_, _| Ok(()));

        assert!(!unsaved::needs_confirmation(&app.session));
        assert!(app.ui.windows.unsaved.is_none());
        assert!(app.awaiting_save.is_none());
        if action == Guarded::Quit {
            assert!(app.quit_confirmed);
        } else {
            assert!(!app.session.has_package());
        }
    }
}

#[test]
fn browser_save_with_an_existing_path_resumes_only_on_success() {
    for success in [false, true] {
        let mut app = app();
        app.session.path = Some("existing.sbs".into());

        PackageSave::save_then(&mut app, Guarded::Quit, |name, _| {
            assert_eq!(name, "existing_sbs.zip");
            if success { Ok(()) } else { Err("download failed".into()) }
        });

        assert_eq!(app.quit_confirmed, success);
        assert_eq!(app.session.modified, !success);
        assert!(app.awaiting_save.is_none());
        assert!(app.ui.windows.package_save.is_none());
    }
}

#[test]
fn browser_save_copy_preserves_the_original_path_and_unsaved_changes() {
    for archive in [false, true] {
        let mut app = app();
        app.session.path = Some("original.sbs".into());
        PackageSave::request(&mut app, true);
        let request = app.ui.windows.package_save.as_mut().unwrap();
        request.archive = archive;
        request.name = "Copy".into();

        PackageSave::finish(&mut app, true, |name, bytes| {
            assert_eq!(name, if archive { "Copy.mcraft" } else { "Copy_sbs.zip" });
            assert!(zip::ZipArchive::new(std::io::Cursor::new(bytes)).is_ok());
            Ok(())
        });

        assert!(app.session.modified);
        assert_eq!(app.session.path.as_deref(), Some("original.sbs"));
        assert!(app.awaiting_save.is_none());
    }
}

#[test]
fn browser_save_dialog_buttons_and_escape_complete_or_cancel_the_close() {
    for button in ["Download", "Cancel", "Close window", "Escape"] {
        let mut app = app();
        let document = app.session.document_id;
        choose_save(&mut app, Guarded::Package { document, reload: false });
        let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(700.0, 350.0)).build_ui_state(
            move |ui, app: &mut MaterialCraftApp| {
                PackageSave::show(app, ui.ctx(), |_, _| {
                    assert_eq!(button, "Download", "dismissing the dialog must not download");
                    Ok(())
                });
            },
            app,
        );
        harness.run();

        if button == "Escape" {
            harness.key_press(egui::Key::Escape);
        } else {
            harness.get_by_label(button).click();
        }
        harness.run();

        let app = harness.state();
        assert!(app.ui.windows.package_save.is_none(), "{button}");
        assert!(app.awaiting_save.is_none(), "{button}");
        assert_eq!(app.session.has_package(), button != "Download", "{button}");
        if button != "Download" {
            assert!(app.session.modified);
        }
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn browser_save_dialog_snapshot() {
    let mut app = app();
    choose_save(&mut app, Guarded::Quit);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(700.0, 350.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_ui_state(|ui, app: &mut MaterialCraftApp| PackageSave::show(app, ui.ctx(), |_, _| Ok(())), app);
    harness.run();

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    harness.render().unwrap().save(directory.join("browser-save-package.png")).unwrap();
    harness.get_by_label("MaterialCraft Project (*.mcraft)").click();
    harness.run();
    harness.render().unwrap().save(directory.join("browser-save-project.png")).unwrap();
}
