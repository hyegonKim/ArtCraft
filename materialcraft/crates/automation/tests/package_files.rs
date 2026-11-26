use materialcraft_automation::{AuthorizedWorkspace, Filesystem, Headless};
use materialcraft_engine::{Session, core::Package};
use serde_json::json;

#[test]
fn reopening_relative_paths_activates_the_existing_document_and_preserves_edits() {
    let root = std::env::temp_dir().join(format!("materialcraft-workspace-reopen-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();

    for extension in ["sbs", "mcraft"] {
        let workspace = AuthorizedWorkspace::new(Some(&root), Some(&root)).unwrap();
        let mut app = Headless::new(Session::new(Package::new("Source")), Filesystem::Workspace(workspace.clone()));
        let filename = format!("package.{extension}");
        app.save(Some(&filename)).unwrap();
        let mut reopened = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(workspace));
        reopened.open(&filename).unwrap();
        let document = reopened.session.document_id;
        reopened.command_run("folder.create", json!({"name": "Unsaved"})).unwrap();
        let history = reopened.session.history();

        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.document_id, document);
        assert_eq!(reopened.session.open_packages().len(), 2);
        assert_eq!(reopened.session.path.as_deref(), Some(filename.as_str()));
        assert!(reopened.session.modified);
        assert!(reopened.session.pkg.folder_paths().contains("Unsaved"));
        assert_eq!(reopened.session.history(), history);

        reopened.command_run("file.newPackage", json!({"name": "Other"})).unwrap();
        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.document_id, document);
        assert_eq!(reopened.session.open_packages().len(), 3);
        assert!(reopened.session.modified);
        assert!(reopened.session.pkg.folder_paths().contains("Unsaved"));
        assert_eq!(reopened.session.history(), history);

        reopened.save(None).unwrap();

        assert!(!reopened.session.modified);
        assert!(root.join(&filename).is_file());
        assert_eq!(reopened.session.path.as_deref(), Some(filename.as_str()));
    }

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn both_formats_save_and_reopen_using_separate_workspace_capabilities() {
    let root = std::env::temp_dir().join(format!("materialcraft-workspace-packages-{}", std::process::id()));
    let input = root.join("input");
    let output = root.join("output");
    std::fs::create_dir_all(&input).unwrap();
    std::fs::create_dir_all(&output).unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    std::fs::write(input.join("shape.svg"), svg).unwrap();
    let workspace = AuthorizedWorkspace::new(Some(&input), Some(&output)).unwrap();
    let mut app = Headless::new(Session::new(Package::new("Package")), Filesystem::Workspace(workspace));
    app.import_bitmap("shape.svg", None).unwrap();
    for extension in ["mcraft", "sbs"] {
        let filename = format!("package.{extension}");
        app.save(Some(&filename)).unwrap();
        app.save(None).unwrap();

        assert!(output.join(&filename).is_file());

        if extension == "mcraft" {
            assert_eq!(std::fs::read_dir(&output).unwrap().count(), 1);
        }
        let workspace = AuthorizedWorkspace::new(Some(&output), Some(&output)).unwrap();
        let mut reopened = Headless::new(Session::new(Package::new("Empty")), Filesystem::Workspace(workspace));
        reopened.open(&filename).unwrap();

        assert_eq!(reopened.session.pkg.resources[0].bytes.as_slice(), svg);

        reopened.save(None).unwrap();
    }

    assert_eq!(std::fs::read_dir(&input).unwrap().count(), 1);
    assert!(app.save(Some("../escape.sbs")).is_err());
    assert!(!root.join("escape.sbs").exists());

    // Release workspace directory handles before Windows removes the test tree
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
