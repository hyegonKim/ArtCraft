use egui::Pos2;
use materialcraft_engine::core::NodeId;

/// The node search popup (Space or Tab)
#[derive(Clone, Debug)]
pub(super) struct NodeSearch {
    pub(super) at: Pos2,
    pub(super) text: String,
    pub(super) sel: usize,
    pub(super) connect: Option<(NodeId, String)>,
    /// Opened from this input (a wire dragged off it): the new node's output feeds it
    pub(super) connect_to: Option<(NodeId, String)>,
    pub(super) focus: bool,
}
