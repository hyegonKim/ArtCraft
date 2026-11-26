use crate::document_path;
use materialcraft_core::Package;

pub(crate) fn normalize_references(package: &mut Package, path: &str) {
    for resource in &mut package.resources {
        if !resource.source.is_empty() {
            resource.source = document_path::resolve(path, &resource.source);
        }
    }

    for instance in package.graphs.iter_mut().flat_map(|graph| graph.instances_mut()) {
        if !instance.package.is_empty() {
            instance.package = document_path::resolve(path, &instance.package);
        }
    }
}

impl crate::Session {
    pub(crate) fn discard_source_changes(&mut self, path: &str) {
        self.linked_sources.remove(path);
        self.pkg.dependencies.remove(path);
        for document in self.documents.values_mut() {
            document.pkg.dependencies.remove(path);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut package = self.pkg.clone();
            // Include inactive dependents so closing a source also refreshes their references
            package.graphs.extend(self.documents.values().flat_map(|document| document.pkg.graphs.iter().cloned()));
            package.dependencies = self.linked_sources.clone();
            for warning in load(&mut package) {
                self.warn(warning);
            }
            self.linked_sources.extend(package.dependencies);
        }
        self.refresh_links_from(std::collections::BTreeSet::from([path.to_string()]));
    }
}

pub(crate) fn load(package: &mut Package) -> Vec<String> {
    use std::{collections::BTreeSet, sync::Arc};
    let references = |package: &Package| {
        package
            .graphs
            .iter()
            .flat_map(|graph| graph.instances())
            .filter(|instance| !instance.package.is_empty())
            .map(|instance| instance.package.clone())
            .collect::<Vec<_>>()
    };
    let mut pending = references(package);
    let mut visited = BTreeSet::new();
    let mut warnings = Vec::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }

        if let Some(source) = package.dependencies.get(&path) {
            pending.extend(references(source));
            continue;
        }

        let mut read = || -> std::result::Result<Package, String> {
            if path.starts_with("sbs://") {
                let bytes = materialcraft_formats::sbs::library::read(&path)?;
                let read = materialcraft_formats::sbs::read_sbs(&bytes, &path, &crate::file_cmds::linked_files(&path)).map_err(|error| error.to_string())?;
                warnings.extend(read.warnings);
                return Ok(read.pkg);
            }
            #[cfg(target_arch = "wasm32")]
            return Err("external file is unavailable in the browser".into());
            #[cfg(not(target_arch = "wasm32"))]
            {
                let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
                let read = materialcraft_formats::sbs::read_sbs(&bytes, &path, &crate::file_cmds::linked_files(&path)).map_err(|error| error.to_string())?;
                warnings.extend(read.warnings);
                Ok(read.pkg)
            }
        };
        match read() {
            Ok(mut source) => {
                normalize_references(&mut source, &path);
                pending.extend(references(&source));
                package.dependencies.insert(path, Arc::new(source));
            }
            Err(error) => warnings.push(format!("Could not resolve {path}: {error}")),
        }
    }
    warnings
}
