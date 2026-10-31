//! Types for `GET /v1/media_files/by_job/{job_token}` — list the media files
//! produced by an inference job.
//!
//! Mirrors storyteller-web's `list_media_files_by_job_handler`. Requires an
//! authenticated session; the server only returns files from the caller's
//! own jobs (other users' jobs and unknown tokens return an empty list).

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use sqlite_identifiers::ids::batch_generation_token::BatchGenerationToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use sqlite_identifiers::ids::prompt_token::PromptToken;

use crate::api_defs::common::responses::media_file_cover_image_details::MediaFileCoverImageDetails;
use crate::api_defs::common::responses::media_links::MediaLinks;
use crate::enums::by_table::media_files::media_file_class::MediaFileClass;
use crate::enums::by_table::media_files::media_file_type::MediaFileType;
use crate::tokens::generic_inference_jobs::InferenceJobToken;

pub const LIST_MEDIA_FILES_BY_JOB_URL_PATH_PREFIX: &str = "/v1/media_files/by_job";

/// Build the request path for a job's media files.
pub fn list_media_files_by_job_url_path(job_token: &InferenceJobToken) -> String {
  format!("{}/{}", LIST_MEDIA_FILES_BY_JOB_URL_PATH_PREFIX, job_token.as_str())
}

#[derive(Serialize, Deserialize)]
pub struct ListMediaFilesByJobSuccessResponse {
  pub success: bool,

  /// The job's output files, oldest first.
  pub media_files: Vec<JobMediaFileInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct JobMediaFileInfo {
  pub token: MediaFileToken,

  /// The coarse-grained class of media file: image, video, etc.
  pub media_class: MediaFileClass,

  /// Type of media will dictate which fields are populated and what
  /// the frontend should display (eg. video player vs audio player).
  /// This is closer in meaning to a "mime type".
  pub media_type: MediaFileType,

  /// If the file was generated as part of a batch, this is the token for the batch.
  pub maybe_batch_token: Option<BatchGenerationToken>,

  /// The foreign key to the prompt used to generate the media, if applicable.
  pub maybe_prompt_token: Option<PromptToken>,

  /// Rich CDN links to the media, including thumbnails, previews, and more.
  pub media_links: MediaLinks,

  /// Information about the cover image. Many media files do not require a cover image,
  /// e.g. image files, video files with thumbnails, audio files, etc.
  /// 3D files require them.
  pub cover_image: MediaFileCoverImageDetails,

  /// The original filename for uploaded files, if they were provided.
  /// In the future we'll provide our own internal optional filenames.
  pub maybe_original_filename: Option<String>,

  /// Duration for audio and video files, if available.
  /// Measured in milliseconds.
  pub maybe_duration_millis: Option<u64>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn url_path_includes_token() {
    let token = InferenceJobToken::new_from_str("jinf_abc123");
    assert_eq!(
      list_media_files_by_job_url_path(&token),
      "/v1/media_files/by_job/jinf_abc123",
    );
  }
}
