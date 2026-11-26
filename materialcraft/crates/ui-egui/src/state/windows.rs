use serde::{Deserialize, Serialize};

/// Which dialogs are open
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Windows {
    pub new_graph: bool,
    pub export: bool,
    pub about: bool,
    pub preferences: bool,
    pub shortcuts: bool,
    pub home: bool,
    #[cfg(any(test, target_arch = "wasm32"))]
    #[serde(skip)]
    pub package_save: Option<crate::web_files::PackageSave>,
    #[serde(skip_deserializing)]
    pub unsaved: Option<crate::unsaved::Guarded>,
}
