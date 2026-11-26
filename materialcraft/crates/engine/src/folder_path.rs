use crate::{Result, commands::bad};
use materialcraft_core::Package;

pub(crate) fn name(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || matches!(value, "." | "..") || value.contains(['/', '\\']) || value.chars().any(char::is_control) {
        return Err(bad("explorer", "names must be nonempty, at most 256 bytes, and contain no path separators or control characters"));
    }
    Ok(value)
}

pub(crate) fn validate(path: &str) -> Result<()> {
    if path.len() > 4096 || path.split('/').count() > 64 {
        return Err(bad("explorer", "folder paths must be at most 4096 bytes and 64 levels deep"));
    }
    if !path.is_empty() {
        for component in path.split('/') {
            if name(component)? != component {
                return Err(bad("explorer", "folder names cannot start or end with whitespace"));
            }
        }
    }
    Ok(())
}

pub(crate) fn require(package: &Package, path: &str) -> Result<()> {
    validate(path)?;
    if !path.is_empty() && !package.folder_paths().contains(path) {
        return Err(bad("explorer", format!("no folder `{path}`")));
    }
    Ok(())
}

pub(crate) fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.into() } else { format!("{parent}/{name}") }
}

pub(crate) fn contains(parent: &str, path: &str) -> bool {
    path == parent || path.strip_prefix(parent).is_some_and(|suffix| suffix.starts_with('/'))
}

pub(crate) fn is_resources(path: &str) -> bool {
    path.split('/').next().is_some_and(|name| name.eq_ignore_ascii_case("Resources"))
}

pub(crate) fn require_graph_folder(path: &str) -> Result<()> {
    // Resources is a reserved asset container; commands must enforce the same rule as Explorer
    if is_resources(path) {
        return Err(bad("explorer", "Resources is reserved for assets"));
    }

    Ok(())
}

pub(crate) fn require_resource_folder(package: &Package, path: &str) -> Result<()> {
    require(package, path)?;
    if !is_resources(path) {
        return Err(bad("explorer", "Assets belong in Resources or one of its subfolders"));
    }

    Ok(())
}

pub(crate) fn require_mutable_folder(path: &str) -> Result<()> {
    if path.eq_ignore_ascii_case("Resources") {
        return Err(bad("explorer", "The root Resources folder is reserved"));
    }

    Ok(())
}

pub(crate) fn unique(package: &Package, parent: &str, base: &str) -> String {
    let occupied = package.folder_paths();
    let free = |name: &str| {
        !(parent.is_empty() && name.eq_ignore_ascii_case("Resources")
            || occupied.contains(&join(parent, name))
            || package.graphs.iter().any(|graph| graph.folder == parent && graph.name == name)
            || package.resources.iter().any(|resource| resource.folder == parent && resource.name == name))
    };
    if free(base) {
        return base.to_string();
    }

    let numbered = base.rsplit_once('_').and_then(|(stem, number)| number.parse::<u64>().ok().map(|number| (stem, number)));

    // One more candidate than occupied items guarantees a free name without an unbounded search
    let attempts = occupied.len().saturating_add(package.graphs.len()).saturating_add(package.resources.len()).saturating_add(1);
    (1..=attempts)
        .map(|index| match numbered.and_then(|(stem, number)| number.checked_add(index as u64).map(|next| (stem, next))) {
            Some((stem, next)) => format!("{stem}_{next}"),
            None => format!("{base}_{index}"),
        })
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| base.to_string())
}
