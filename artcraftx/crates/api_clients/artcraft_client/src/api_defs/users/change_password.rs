use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChangePasswordRequest {
  pub password: String,
  pub password_confirmation: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChangePasswordResponse {
  pub success: bool,
}
