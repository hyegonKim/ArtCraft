use serde::{Deserialize, Serialize};

use super::{GraphCamera, ImageCamera, Prefs, Windows};
use crate::docking::Dock;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UiState {
    pub dock: Dock,
    pub prefs: Prefs,
    pub windows: Windows,
    #[serde(skip)]
    pub explorer: crate::explorer::ExplorerState,
    pub graph_camera: GraphCamera,
    pub image_camera: ImageCamera,
    /// Library: the selected category path ("" = all) and the search text
    pub library_category: String,
    pub library_search: String,
    /// Library entry ids starred as Favorites, in the order they were added
    pub library_favorites: Vec<String>,
    /// Expanded library categories
    pub library_expanded: std::collections::BTreeSet<String>,
    /// Library filters: entries MaterialCraft doesn't have yet, and entries for other graph types
    pub library_show_missing: bool,
    pub library_show_other: bool,
    /// The library entry last clicked
    #[serde(skip)]
    pub library_selected: Option<String>,
    /// Recently opened or saved packages, newest first
    pub recent: Vec<String>,
    /// New graph dialog fields
    pub new_graph_kind: materialcraft_engine::core::GraphKind,
    pub new_graph_template: String,
    pub new_graph_name: String,
    pub new_graph_size: u32,
    /// Export window: the saved settings, the open tab and the copy the open window edits
    pub export: crate::export::ExportSettings,
    pub export_tab: crate::export::ExportTab,
    #[serde(skip)]
    pub export_draft: Option<crate::export::ExportSettings>,
    /// Frame the whole graph on the next frame
    #[serde(skip)]
    pub frame_all: bool,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            dock: Dock::default(),
            prefs: Prefs::default(),
            windows: Windows::default(),
            explorer: Default::default(),
            graph_camera: GraphCamera::default(),
            image_camera: ImageCamera::default(),
            library_category: String::new(),
            library_search: String::new(),
            library_favorites: Vec::new(),
            library_expanded: Default::default(),
            library_show_missing: true,
            library_show_other: true,
            library_selected: None,
            recent: Vec::new(),
            new_graph_kind: materialcraft_engine::core::GraphKind::Material,
            new_graph_template: "pbr".into(),
            new_graph_name: "New_graph".into(),
            new_graph_size: 1024,
            export: crate::export::ExportSettings::default(),
            export_tab: crate::export::ExportTab::default(),
            export_draft: None,
            frame_all: true,
        }
    }
}

impl UiState {
    /// Remember `path` at the top of the recent list
    pub fn add_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_string());
        self.recent.truncate(12);
    }
}
