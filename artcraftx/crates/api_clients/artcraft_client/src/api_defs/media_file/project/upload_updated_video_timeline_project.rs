use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const UPLOAD_UPDATED_VIDEO_TIMELINE_PROJECT_URL_PATH: &str = "/v1/media_files/upload/project/video_timeline/update/{token}";

/// For the URL PathInfo
#[derive(Serialize, Deserialize)]
pub struct UploadUpdatedVideoTimelineProjectPathInfo {
  pub token: MediaFileToken,
}

/// Response for overwriting an existing video editor timeline project.
#[derive(Serialize, Deserialize)]
pub struct UploadUpdatedVideoTimelineProjectSuccessResponse {
  pub success: bool,

  /// Token of the updated project media file (unchanged by the update).
  pub media_file_token: MediaFileToken,
}
