use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api_defs::tags::common::TagDetails;

// ── GET /v1/tags/media_file/list/{media_file_token} ──

#[derive(Deserialize)]
pub struct ListMediaFileTagsPathInfo {
  pub media_file_token: MediaFileToken,
}

#[derive(Serialize)]
pub struct ListMediaFileTagsSuccessResponse {
  pub success: bool,

  /// All (live) tags on the media file, sorted by tag value.
  pub tags: Vec<TagDetails>,
}
