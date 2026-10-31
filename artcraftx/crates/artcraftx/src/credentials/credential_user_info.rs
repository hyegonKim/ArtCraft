use serde_derive::{Deserialize, Serialize};

/// Optional identity metadata attached to a credential, so users can tell
/// multiple accounts for the same service apart.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CredentialUserInfo {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub username: Option<String>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub email: Option<String>,
}
