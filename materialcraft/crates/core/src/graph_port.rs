use serde::{Deserialize, Serialize};

/// A named connector retained when an instance's source is unavailable
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphPort {
    pub identifier: String,
    pub type_code: u32,
}
