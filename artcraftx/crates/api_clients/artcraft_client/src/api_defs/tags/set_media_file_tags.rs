use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api_defs::tags::common::TagDetails;

// ── POST /v1/tags/media_file/set/{media_file_token} ──

#[derive(Deserialize)]
pub struct SetMediaFileTagsPathInfo {
  pub media_file_token: MediaFileToken,
}

/// Exactly one of `maybe_tags` / `maybe_tags_list` must be set (400 if
/// both or neither). Tag text is trimmed and deduped case-insensitively;
/// empty entries are dropped. Unlike `add`, sanitizing down to zero tags
/// is allowed — it clears all tags from the media file.
#[derive(Deserialize)]
pub struct SetMediaFileTagsRequest {
  /// Comma-separated tags, e.g. `"cats, Sci-Fi, wallpaper"`.
  pub maybe_tags: Option<String>,

  /// Tags as a list. Entries are still trimmed.
  pub maybe_tags_list: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct SetMediaFileTagsSuccessResponse {
  pub success: bool,

  /// The media file's full tag set after the operation, with canonical
  /// tokens and fresh use counts.
  pub tags: Vec<TagDetails>,

  /// How many previously-attached tag links were removed because they
  /// weren't mentioned in the request. (Orphaned tags are not deleted.)
  pub removed_count: u64,
}
