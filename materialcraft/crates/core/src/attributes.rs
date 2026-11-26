use serde::{Deserialize, Serialize};

/// Descriptive metadata ("Attributes")
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Attributes {
    pub label: String,
    pub category: String,
    pub description: String,
    pub author: String,
    pub author_url: String,
    pub tags: String,
    pub group: String,
    pub user_data: String,
}
