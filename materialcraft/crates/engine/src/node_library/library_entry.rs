use super::{LibraryCanvas, LibraryItem};
use serde::Serialize;

/// One selectable library entry and its availability on a destination canvas.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LibraryEntry {
    /// Stable id, e.g. `node:blur`, `function:add`, `sbs://functions.sbs#Functions/Math/Pi`
    pub id: String,
    pub label: String,
    /// `Top` or `Top/Sub`, from [`super::CATEGORIES`]
    pub category: String,
    pub description: String,
    #[serde(flatten)]
    pub item: LibraryItem,
    pub canvas: LibraryCanvas,
    /// Why the entry can't be added yet (the package it needs is not in MaterialCraft)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
    /// It can be added, but is incomplete (it uses operations MaterialCraft lacks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}
