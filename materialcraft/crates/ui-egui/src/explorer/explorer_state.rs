use super::ExplorerTarget;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExplorerState {
    pub selected: Option<ExplorerTarget>,
    pub rename: Option<(ExplorerTarget, String)>,
    /// The pointer was over the Explorer last frame, so Enter, F2 and Delete act on its selection
    #[serde(skip)]
    pub hovered: bool,
}
