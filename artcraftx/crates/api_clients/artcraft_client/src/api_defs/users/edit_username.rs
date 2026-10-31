use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditUsernameRequest {
  pub display_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct EditUsernameResponse {
  pub success: bool,
}
