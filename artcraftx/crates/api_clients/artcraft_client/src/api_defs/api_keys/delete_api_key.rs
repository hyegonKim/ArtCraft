use serde_derive::Serialize;

// ── DELETE /v1/api_keys/{api_key_token} ──

#[derive(Serialize)]
pub struct DeleteApiKeySuccessResponse {
  pub success: bool,
}
