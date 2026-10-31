use serde_derive::{Deserialize, Serialize};

use crate::tokens::tags::TagToken;

// ── DELETE /v1/tags/{tag_token} ──

#[derive(Deserialize)]
pub struct DeleteTagPathInfo {
  pub tag_token: TagToken,
}

#[derive(Serialize)]
pub struct DeleteTagSuccessResponse {
  pub success: bool,

  /// How many media-file links were hard-deleted along with the tag.
  /// (The tag record itself is soft-deleted.)
  pub removed_link_count: u64,
}
