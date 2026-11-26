use materialcraft_formats::sbs::LayoutItem;

/// A layout placement, before it is resolved against the loaded packages
#[derive(Clone, Debug)]
pub(super) struct LibraryPlacement {
    pub(super) category: String,
    pub(super) item: LayoutItem,
}
