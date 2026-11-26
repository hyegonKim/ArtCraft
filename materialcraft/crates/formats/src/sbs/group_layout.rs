use super::{sbs_writer::SbsWriter, xml::xml_escape};
use crate::{FormatError, Result};
use materialcraft_core::Package;
use std::{collections::BTreeSet, fmt::Write};

pub(super) fn write(out: &mut String, package: &Package, writer: &mut SbsWriter, self_uid: u64) -> Result<()> {
    let folders = package.folder_paths();
    // Bound recursion for hostile package folders supplied through commands or imported XML
    if folders.iter().any(|path| path.split('/').count() > 64 || path.split('/').any(str::is_empty)) {
        return Err(FormatError::BadPackage("SBS folders must be nonempty and at most 64 levels deep".into()));
    }

    write_level(out, package, writer, self_uid, &folders, "")
}

fn write_level(out: &mut String, package: &Package, writer: &mut SbsWriter, self_uid: u64, folders: &BTreeSet<String>, parent: &str) -> Result<()> {
    let prefix = if parent.is_empty() { String::new() } else { format!("{parent}/") };
    let mut written = BTreeSet::new();
    for graph in &package.graphs {
        if graph.folder == parent {
            if graph.function.is_some() {
                super::standalone_function::write(out, graph, writer)?;
            } else {
                writer.write_graph(out, package, graph, self_uid)?;
            }
        } else if let Some(child) = graph.folder.strip_prefix(&prefix).and_then(|path| path.split('/').next())
            && written.insert(child.to_string())
        {
            write_folder(out, package, writer, self_uid, folders, &prefix, child)?;
        }
    }
    for resource in package.resources.iter().filter(|resource| resource.folder == parent) {
        super::resource::write(out, resource, writer)?;
    }
    for path in folders {
        if let Some(child) = path.strip_prefix(&prefix).filter(|path| !path.contains('/'))
            && written.insert(child.to_string())
        {
            write_folder(out, package, writer, self_uid, folders, &prefix, child)?;
        }
    }
    Ok(())
}

fn write_folder(
    out: &mut String,
    package: &Package,
    writer: &mut SbsWriter,
    self_uid: u64,
    folders: &BTreeSet<String>,
    prefix: &str,
    name: &str,
) -> Result<()> {
    let _ = write!(out, "<group><uid v=\"{}\"/><identifier v=\"{}\"/>", writer.next_uid(), xml_escape(name));
    if let Some(description) = package.folder_descriptions.get(&format!("{prefix}{name}")).filter(|text| !text.is_empty()) {
        let _ = write!(out, "<desc v=\"{}\"/>", xml_escape(description));
    }
    out.push_str("<content>");
    write_level(out, package, writer, self_uid, folders, &format!("{prefix}{name}"))?;
    out.push_str("</content></group>");
    Ok(())
}
