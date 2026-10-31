use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

// ── POST /v1/tags/media_file/clear/{media_file_token} ──

#[derive(Deserialize)]
pub struct ClearMediaFileTagsPathInfo {
  pub media_file_token: MediaFileToken,
}

#[derive(Serialize)]
pub struct ClearMediaFileTagsSuccessResponse {
  pub success: bool,

  /// How many tag links were removed from the media file. (Orphaned
  /// tags are not deleted.)
  pub removed_count: u64,
}
