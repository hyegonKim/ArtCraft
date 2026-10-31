use serde_derive::{Deserialize, Serialize};

use crate::tokens::api_keys::ApiKeyToken;

// ── POST /v1/api_keys/create ──

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
  /// Display name (title) for the key.
  pub name: String,

  /// Optional description.
  pub maybe_description: Option<String>,
}

#[derive(Serialize)]
pub struct CreateApiKeySuccessResponse {
  pub success: bool,

  /// The key's `token` — used for all subsequent management
  /// (get / update / delete). Safe to store and display.
  pub api_key_token: ApiKeyToken,

  /// The full secret API key value. This is the ONLY time it is ever
  /// returned — the caller must store it now, as it cannot be retrieved again.
  pub api_key: String,
}
