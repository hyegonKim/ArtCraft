use crate::ResourceBytes;
use serde::{Deserialize, Serialize};

/// An asset retained in memory until the package is saved: a bitmap or an SVG.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    /// Unique name, referenced by nodes (`resource` parameter)
    pub name: String,
    #[serde(default = "default_folder")]
    pub folder: String,
    /// File name inside the package's `resources/` folder
    pub file: String,
    /// Unavailable external asset retained for repair on a later load
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Original bytes, saved beside native SBS or inside a MaterialCraft archive.
    #[serde(skip)]
    pub bytes: ResourceBytes,
}

impl Resource {
    /// A resource named `name`, stored as `file` in the package
    pub fn new(name: impl Into<String>, file: impl Into<String>, bytes: ResourceBytes) -> Resource {
        Resource { name: name.into(), folder: default_folder(), file: file.into(), source: String::new(), bytes }
    }

    pub fn sbs_path(&self) -> String {
        if self.folder.is_empty() { self.name.clone() } else { format!("{}/{}", self.folder, self.name) }
    }
}

fn default_folder() -> String {
    "Resources".into()
}
