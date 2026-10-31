//! Binding for `GET /v1/media_files/by_job/{job_token}` — list the media
//! files produced by an inference job.

use crate::api_defs::media_file::list_media_files_by_job::{list_media_files_by_job_url_path, ListMediaFilesByJobSuccessResponse};
use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::error::storyteller_error::StorytellerError;
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use crate::utils::api_host::ApiHost;
use crate::utils::basic_json_get_request::basic_json_get_request;

/// List the media files a completed job produced.
/// Requires an authenticated session (cookie or API key); the server only
/// returns files from the caller's own jobs — other users' jobs and unknown
/// tokens come back as an empty list.
pub async fn list_media_files_by_job(
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  job_token: &InferenceJobToken,
) -> Result<ListMediaFilesByJobSuccessResponse, StorytellerError> {
  basic_json_get_request(
    api_host,
    &list_media_files_by_job_url_path(job_token),
    Some(creds),
  ).await
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Load the desktop app's stored production credential cookie header.
  fn load_artcraft_cookie_creds() -> StorytellerCredentialSet {
    let home = std::env::var("HOME").expect("HOME not set");
    let path = format!("{home}/Artcraft/artcraftx/credentials/artcraft.toml");
    let contents = std::fs::read_to_string(&path).expect("no production credential on disk");
    let value: toml::Value = toml::from_str(&contents).expect("credential file should parse");
    let header = value
        .get("cookie").and_then(|c| c.get("cookie_header"))
        .and_then(|h| h.as_str())
        .expect("credential file has no cookie header");
    StorytellerCredentialSet::parse_multi_cookie_header(header)
        .expect("cookie header should parse")
        .expect("cookie header should contain session cookies")
  }

  #[tokio::test]
  #[ignore] // Live: hits api.storyteller.ai with the stored credential (read-only, no credits)
  async fn live_list_media_files_by_job() {
    let host = ApiHost::Storyteller;
    let creds = load_artcraft_cookie_creds();
    // A known completed job (generic midjourney smoke test).
    let token = InferenceJobToken::new_from_str("jinf_sxr641zj1qgfaky9j78hwerk1r2");
    let result = list_media_files_by_job(&host, &creds, &token).await.unwrap();

    println!("media_files: {} entries", result.media_files.len());
    for file in &result.media_files {
      println!("media file {}: class={:?} cdn={}", file.token.as_str(), file.media_class, file.media_links.cdn_url);
    }

    // NB: Not all generation paths populate the media-file -> source-job
    // linkage, so an empty list is valid for some completed jobs.
    assert!(result.success);
  }
}
