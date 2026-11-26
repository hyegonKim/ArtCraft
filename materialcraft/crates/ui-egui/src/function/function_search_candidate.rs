use super::function_library_drag::FunctionLibraryDrag;

/// One search result: its label, what it is (shown dimmed on the right) and what adding it does
#[derive(Clone, Debug)]
pub(crate) struct FunctionSearchCandidate {
    pub(crate) label: String,
    pub(crate) group: String,
    pub(crate) entry: FunctionLibraryDrag,
}
