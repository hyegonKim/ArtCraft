use serde::Serialize;

/// The graph kind that can take an entry
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryCanvas {
    Any,
    Material,
    Function,
    Pattern,
}
