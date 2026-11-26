use super::{SBS_ENTRY, bad, references, remap};
use crate::{Result, sbs};
use materialcraft_core::{Package, ResourceBytes};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Write};

const MAX_RESOURCE_STEM_CHARS: usize = 40;
const MAX_RESOURCE_EXTENSION_CHARS: usize = 16;

/// Files relative to the directory containing `entry`. Assets keep their original bytes.
pub struct ProjectFiles {
    pub entry: String,
    pub files: BTreeMap<String, ResourceBytes>,
}

impl ProjectFiles {
    pub fn build(package: &Package, filename: &str, archive: bool) -> Result<Self> {
        let filename = crate::safe_file_name(filename);
        let stem = filename.rsplit_once('.').map_or(filename.as_str(), |(stem, _)| stem);
        let entry = if archive { SBS_ENTRY.to_string() } else { format!("{stem}.sbs") };

        let mut names = BTreeMap::new();
        let mut pending = references(package);
        let mut visited = BTreeSet::new();
        while let Some(path) = pending.pop() {
            if !visited.insert(path.clone()) || (!archive && !path.starts_with("mcraft://")) {
                continue;
            }

            // Installed library aliases remain valid without copying the whole standard library
            let Some(source) = package.dependencies.get(&path) else {
                if path.starts_with("sbs://") {
                    continue;
                }

                return Err(bad(format!("Cannot package missing dependency `{path}`")));
            };

            names.insert(path, format!("resources/packages/{}.sbs", names.len() + 1));
            pending.extend(references(source));
        }

        // Dependency edits need a new directory so saved inter-package references remain intact
        let mut digest = std::collections::hash_map::DefaultHasher::new();
        for source in names.keys() {
            source.hash(&mut digest);
            let source = package.dependencies.get(source).ok_or_else(|| bad("Missing package dependency"))?;
            sbs::write_sbs(source)?.hash(&mut digest);
        }
        let directory = format!("resources/packages/{:016x}", digest.finish());
        for (index, file) in names.values_mut().enumerate() {
            *file = format!("{directory}/{}.sbs", index + 1);
        }

        let mut result = Self { entry: entry.clone(), files: BTreeMap::new() };
        result.add_package(package, &entry, &names)?;
        for (source, file) in &names {
            let package = package.dependencies.get(source).ok_or_else(|| bad("Package dependency disappeared"))?;
            result.add_package(package, file, &names)?;
        }

        Ok(result)
    }

    fn add_package(&mut self, package: &Package, file: &str, names: &BTreeMap<String, String>) -> Result<()> {
        let mut package = package.clone();
        let folder = file.rsplit_once('/').map_or("", |(folder, _)| folder);
        let local_names = names
            .iter()
            .map(|(old, new)| {
                let new = if folder.is_empty() { new.clone() } else { new.strip_prefix(&format!("{folder}/")).unwrap_or(new).to_string() };
                (old.clone(), new)
            })
            .collect();
        remap(&mut package, &local_names);

        for resource in &mut package.resources {
            if resource.bytes.is_empty() {
                if !resource.source.is_empty() {
                    return Err(bad(format!("Cannot save missing resource `{}`: {}", resource.name, resource.source)));
                }

                return Err(bad(format!("Resource `{}` has no data", resource.name)));
            }

            let name = crate::safe_file_name(&resource.file);
            let extension = name.rsplit_once('.').map_or("bin", |(_, extension)| extension);
            let stem = crate::safe_file_name(&resource.name).chars().take(MAX_RESOURCE_STEM_CHARS).collect::<String>().to_ascii_lowercase();
            let extension = extension.chars().take(MAX_RESOURCE_EXTENSION_CHARS).collect::<String>().to_ascii_lowercase();

            // Lowercase paths match archive duplicate checks without changing resource identifiers
            // Content hashes prevent failed saves or sibling packages from overwriting saved assets
            let relative = format!("resources/{stem}-{:016x}.{extension}", resource.bytes.content_hash());
            let path = if folder.is_empty() { relative.clone() } else { format!("{folder}/{relative}") };
            if let Some(existing) = self.files.get(&path) {
                if existing != &resource.bytes {
                    return Err(bad("Resource filename collision"));
                }
            } else {
                self.files.insert(path, resource.bytes.clone());
            }

            resource.source = relative;
            resource.bytes = ResourceBytes::default();
        }

        self.files.insert(file.into(), ResourceBytes::new(sbs::write_sbs(&package)?));
        Ok(())
    }

    pub fn zip(&self) -> Result<Vec<u8>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.add_directory("resources/", zip::write::SimpleFileOptions::default()).map_err(|error| bad(error.to_string()))?;
        for (name, bytes) in &self.files {
            let is_text = name.ends_with(".sbs") || name.ends_with(".svg");
            let compression_method = if is_text { zip::CompressionMethod::Deflated } else { zip::CompressionMethod::Stored };

            // ZIP64 prevents a separate 4 GiB entry ceiling
            let options = zip::write::SimpleFileOptions::default().compression_method(compression_method).large_file(true);
            writer.start_file(name, options).map_err(|error| bad(error.to_string()))?;
            writer.write_all(bytes.as_slice()).map_err(|error| bad(error.to_string()))?;
        }

        Ok(writer.finish().map_err(|error| bad(error.to_string()))?.into_inner())
    }

    pub fn xml(&self) -> Result<&[u8]> {
        self.files.get(&self.entry).map(ResourceBytes::as_slice).ok_or_else(|| bad("The entry SBS is missing"))
    }
}
