use materialcraft_core::Package;

#[derive(Clone, Debug)]
pub struct ExplorerClipboard {
    pub(crate) source: Package,
    pub(crate) document: u64,
    pub(crate) path: Option<String>,
    pub(crate) graph: Option<usize>,
    pub(crate) folder: Option<String>,
}
