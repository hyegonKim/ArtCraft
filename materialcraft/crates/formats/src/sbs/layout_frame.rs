use super::LayoutItem;

/// A titled frame and the nodes inside it, top to bottom
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutFrame {
    pub title: String,
    /// The frame groups function nodes (it sits in a function graph or a parameter function)
    pub function: bool,
    pub items: Vec<LayoutItem>,
}
