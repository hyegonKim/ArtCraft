use serde_derive::{Deserialize, Serialize};

/// The generation modalities that each have a prompt box.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptboxModality {
  Image,
  Video,
  Mesh,
  Splat,
  Audio,
}
