use serde_derive::Serialize;

use crate::api_defs::api_keys::common::ApiKeyInfo;

// ── GET /v1/api_keys/{api_key_token} ──

#[derive(Serialize)]
pub struct GetApiKeySuccessResponse {
  pub success: bool,
  pub api_key: ApiKeyInfo,
}
