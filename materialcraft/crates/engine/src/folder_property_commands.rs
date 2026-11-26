use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, always, bad, strp},
    folder_path,
};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("folder.properties", "Inspect folder properties", [], None, "{folder, document?}", always, properties),
        cmd!("folder.attributes", "Folder attributes", [], None, "{folder, document?, identifier?, description?}", always, attributes),
    ]
}

fn target<'a>(session: &Session, parameters: &'a Value) -> Result<(u64, &'a str)> {
    let document = match parameters.get("document") {
        None | Some(Value::Null) => session.document_id,
        Some(value) => value.as_u64().ok_or_else(|| bad("folder", "document must be an open document id"))?,
    };
    let folder = strp(parameters, "folder").filter(|folder| !folder.is_empty()).ok_or_else(|| bad("folder", "folder is required"))?;
    folder_path::require(session.package(document)?, folder)?;
    Ok((document, folder))
}

fn properties(session: &mut Session, parameters: &Value) -> Result<Value> {
    let (document, folder) = target(session, parameters)?;
    Ok(json!({"document": document, "folder": folder, "identifier": folder.rsplit('/').next().unwrap_or(folder),
        "description": session.package(document)?.folder_descriptions.get(folder).map(String::as_str).unwrap_or(""),
        "reserved": folder.eq_ignore_ascii_case("Resources")}))
}

fn attributes(session: &mut Session, parameters: &Value) -> Result<Value> {
    let (document, folder) = target(session, parameters)?;
    let description = parameters
        .get("description")
        .map(|value| value.as_str().filter(|value| value.len() <= 65536).ok_or_else(|| bad("folder.attributes", "description must be text of at most 64 KiB")))
        .transpose()?;
    let new = parameters
        .get("identifier")
        .map(|value| {
            let name = value.as_str().ok_or_else(|| bad("folder.attributes", "identifier must be text"))?;
            let name = folder_path::name(name)?;
            Ok::<_, crate::EngineError>(folder_path::join(folder.rsplit_once('/').map_or("", |(parent, _)| parent), name))
        })
        .transpose()?;
    session.edit_package(document, "Folder attributes", None, |package| {
        let mut path = folder;
        if let Some(new) = &new {
            if new != folder {
                crate::folder_commands::relocate_in_package(package, folder, new)?;
            }
            path = new;
        }
        if let Some(description) = description {
            if description.is_empty() {
                package.folder_descriptions.remove(path);
            } else {
                package.folder_descriptions.insert(path.into(), description.into());
            }
        }
        Ok(json!({"folder": path}))
    })
}
