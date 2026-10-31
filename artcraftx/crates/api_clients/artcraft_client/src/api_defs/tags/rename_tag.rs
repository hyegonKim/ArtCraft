use serde_derive::{Deserialize, Serialize};

use crate::tokens::tags::TagToken;

use crate::api_defs::tags::common::TagDetails;

// ── PUT /v1/tags/rename/{tag_token} ──

#[derive(Deserialize)]
pub struct RenameTagPathInfo {
  pub tag_token: TagToken,
}

#[derive(Deserialize)]
pub struct RenameTagRequest {
  /// The new tag text (trimmed server-side). Both case-only changes and
  /// wholesale renames are allowed. Fails with 400 if the caller already
  /// has a different tag with the same lowercased value.
  pub new_tag_value: String,
}

#[derive(Serialize)]
pub struct RenameTagSuccessResponse {
  pub success: bool,

  /// The tag after the rename.
  pub tag: TagDetails,
}
