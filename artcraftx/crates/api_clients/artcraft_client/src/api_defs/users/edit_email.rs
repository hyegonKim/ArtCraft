use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct EditEmailRequest {
  pub email_address: String,
}

#[derive(Serialize, Deserialize)]
pub struct EditEmailResponse {
  pub success: bool,
}
