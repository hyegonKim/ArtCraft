use serde_derive::{Deserialize, Serialize};

use crate::api_defs::tags::common::TagDetails;

// ── GET /v1/tags/list ──

#[derive(Deserialize)]
pub struct ListTagsQueryParams {
  pub cursor: Option<String>,
  pub limit: Option<u32>,
}

#[derive(Serialize)]
pub struct ListTagsSuccessResponse {
  pub success: bool,

  /// The logged-in user's (live) tags, newest first.
  pub tags: Vec<TagDetails>,

  /// Present when there may be more results; pass back as `cursor`.
  pub maybe_cursor: Option<String>,
}
