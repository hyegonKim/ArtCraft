use serde_derive::{Deserialize, Serialize};

use crate::api_defs::api_keys::common::ApiKeyInfo;

// ── GET /v1/api_keys/list ──

#[derive(Deserialize)]
pub struct ListApiKeysQueryParams {
  /// Page size. Defaults server-side if omitted.
  pub limit: Option<u32>,

  /// Number of rows to skip. Defaults to 0.
  pub offset: Option<u32>,
}

#[derive(Serialize)]
pub struct ListApiKeysSuccessResponse {
  pub success: bool,

  /// The user's API keys (including soft-deleted ones), newest first. Each
  /// entry's key value is truncated — the full secret is never listed.
  pub api_keys: Vec<ApiKeyInfo>,
}
