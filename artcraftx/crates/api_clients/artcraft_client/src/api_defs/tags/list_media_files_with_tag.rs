use serde_derive::Deserialize;

use crate::tokens::tags::TagToken;

// ── GET /v1/tags/media_files/with_tag/{tag_token} ──
//
// NB: The success response lives in `storyteller_web`'s handler because
// the wire shape embeds `MediaLinks` / `MediaFileCoverImageDetails`
// constructors that depend on the request's `MediaDomain` +
// `ServerEnvironment`.

#[derive(Deserialize)]
pub struct ListMediaFilesWithTagPathInfo {
  pub tag_token: TagToken,
}

#[derive(Deserialize)]
pub struct ListMediaFilesWithTagQueryParams {
  pub cursor: Option<String>,
  pub limit: Option<u32>,
}
