use super::search_wire::SearchWire;
use egui::Pos2;

#[derive(Clone, Debug)]
pub(crate) struct FunctionSearch {
    /// Where the new node goes, in canvas coordinates
    pub(crate) at: Pos2,
    pub(crate) text: String,
    pub(crate) selected_index: usize,
    pub(crate) wire: Option<SearchWire>,
    pub(crate) focus: bool,
}

impl FunctionSearch {
    pub(crate) fn at(at: Pos2, wire: Option<SearchWire>) -> Self {
        Self { at, text: String::new(), selected_index: 0, wire, focus: true }
    }
}
