use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, has_package},
};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.reload",
            "Reload package",
            [],
            None,
            "{discard?: bool} reloads the current package; unsaved changes require explicit discard",
            |session| { session.path.as_ref().map(|_| ()).ok_or_else(|| "the package has no file to reload".into()) },
            reload
        ),
        cmd!(
            "file.saveCopyAs",
            "Save a copy as...",
            [],
            None,
            "{path} writes an SBS or MaterialCraft copy without changing the current path or saved state",
            has_package,
            save_copy
        ),
    ]
}

fn reload(session: &mut Session, parameters: &Value) -> Result<Value> {
    if session.modified && parameters.get("discard").and_then(Value::as_bool) != Some(true) {
        return Err(bad("file.reload", "save the package or explicitly discard its edits"));
    }

    let path = session.path.clone().ok_or_else(|| bad("file.reload", "the package has no file to reload"))?;
    let bytes = read_package(&path)?;
    let linked = crate::file_cmds::linked_files(&path);
    let mut read = crate::file_cmds::read_package_bytes(&bytes, &path, &linked)?;
    for (path, source) in &session.pkg.dependencies {
        read.pkg.dependencies.entry(path.clone()).or_insert_with(|| source.clone());
    }
    let safe_path = read.warnings.is_empty().then_some(path.clone());
    read.warnings.extend(crate::linked_files::load(&mut read.pkg));

    crate::file_cmds::replace_package(session, read.pkg, safe_path);
    session.load_warnings = read.warnings;
    session.refresh_links();
    session.modified = false;
    Ok(json!({"path": session.path, "warnings": session.load_warnings}))
}

fn read_package(path: &str) -> Result<Vec<u8>> {
    if path.starts_with("sbs://") {
        return materialcraft_formats::sbs::library::read(path).map_err(|error| bad("file.reload", error));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read(path).map_err(|error| bad("file.reload", error.to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    Err(bad("file.reload", "reopen local browser files with the file picker"))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_copy(session: &mut Session, parameters: &Value) -> Result<Value> {
    let path = crate::commands::strp(parameters, "path")
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| bad("file.saveCopyAs", "a nonempty path is required"))?;
    let path = crate::document_path::normalize(&crate::file_cmds::package_path(path));
    if path.contains("://") || session.open_packages().iter().any(|entry| entry.2 == Some(path.as_str())) {
        return Err(bad("file.saveCopyAs", "choose a local file that is not already open"));
    }

    let bytes = crate::file_cmds::write_package_at(session, &path)?;
    Ok(json!({"path": path, "bytes": bytes}))
}

#[cfg(target_arch = "wasm32")]
fn save_copy(_: &mut Session, _: &Value) -> Result<Value> {
    Err(bad("file.saveCopyAs", "the browser saves copies with a download"))
}
