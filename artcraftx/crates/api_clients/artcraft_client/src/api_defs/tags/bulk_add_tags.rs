use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api_defs::tags::common::TagDetails;

// ── POST /v1/tags/bulk_add ──

/// Exactly one of `maybe_tags` / `maybe_tags_list` must be set (400 if
/// both or neither). Tag text is trimmed and deduped case-insensitively;
/// empty entries are dropped. Must sanitize to at least one tag (400
/// otherwise). Media file tokens are deduped; tokens the user doesn't
/// own (or that are deleted) are silently skipped.
#[derive(Deserialize)]
pub struct BulkAddTagsRequest {
  /// Media files to tag.
  pub media_file_tokens: Vec<MediaFileToken>,

  /// Comma-separated tags, e.g. `"cats, Sci-Fi, wallpaper"`.
  pub maybe_tags: Option<String>,

  /// Tags as a list. Entries are still trimmed.
  pub maybe_tags_list: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct BulkAddTagsSuccessResponse {
  pub success: bool,

  /// The subset of the input tokens that were actually tagged: files
  /// that exist, aren't deleted, and are owned by the caller.
  pub accepted_media_file_tokens: Vec<MediaFileToken>,

  /// The tags from this request after upsert, with canonical tokens and
  /// fresh use counts.
  pub tags: Vec<TagDetails>,
}
