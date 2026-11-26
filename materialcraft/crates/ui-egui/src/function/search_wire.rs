/// The wire a search was opened from
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SearchWire {
    /// Dragged from this node's output: the new node takes it on its first input
    From(u32),
    /// Dragged from this node's input slot: the new node's output feeds it
    To(u32, usize),
}
