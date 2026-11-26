use std::collections::{BTreeMap, BTreeSet};

use materialcraft_engine::ColorSpace;
use serde::{Deserialize, Serialize};

use super::MaterialExportSettings;

/// The Export window's tabs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportTab {
    /// The open graph's outputs as image files
    #[default]
    FromGraph,
    /// The outputs of several graphs as image files
    Batch,
    /// A MaterialX document with its textures in a folder (#190)
    MaterialX,
    /// The same files as MaterialX in one .mcmat archive (#190)
    Mcmat,
}

impl ExportTab {
    pub const ALL: [ExportTab; 4] = [ExportTab::FromGraph, ExportTab::Batch, ExportTab::MaterialX, ExportTab::Mcmat];

    pub fn label(self) -> &'static str {
        match self {
            ExportTab::FromGraph => "From graph",
            ExportTab::Batch => "Batch",
            ExportTab::MaterialX => "MaterialX",
            ExportTab::Mcmat => "MCMAT",
        }
    }

    /// The tabs that publish a material rather than image files
    pub fn publishes_material(self) -> bool {
        matches!(self, ExportTab::MaterialX | ExportTab::Mcmat)
    }
}

/// What the Export window remembers between exports (Save settings)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportSettings {
    /// Folder the image files are written to
    pub destination: String,
    /// File extension from the window's format list
    pub format: String,
    /// File names with $(graph), $(identifier) and $(usage)
    pub pattern: String,
    /// 8, 16 or 32 bits; 0 keeps each output's format
    pub depth: u32,
    /// Unchecked outputs, as `graph/identifier`
    pub skipped_outputs: BTreeSet<String>,
    /// Color spaces picked per `graph/identifier`; the others follow their usage
    pub color_spaces: BTreeMap<String, ColorSpace>,
    /// Export again whenever the outputs change
    pub automatic: bool,
    pub batch: BatchExportSettings,
    pub material: MaterialExportSettings,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            destination: String::new(),
            format: "png".into(),
            pattern: "$(graph)_$(identifier)".into(),
            depth: 0,
            skipped_outputs: BTreeSet::new(),
            color_spaces: BTreeMap::new(),
            automatic: false,
            batch: BatchExportSettings::default(),
            material: MaterialExportSettings::default(),
        }
    }
}

/// The Batch tab's output size; a graph is exported when any of its outputs is checked
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BatchExportSettings {
    /// Output width and height in pixels; 0 keeps each graph's size
    pub width: u32,
    pub height: u32,
    /// Width and height change together
    pub linked: bool,
}

impl Default for BatchExportSettings {
    fn default() -> Self {
        Self { width: 2048, height: 2048, linked: true }
    }
}

/// The key an output's choices are kept under
pub fn output_key(graph: &str, identifier: &str) -> String {
    format!("{graph}/{identifier}")
}
