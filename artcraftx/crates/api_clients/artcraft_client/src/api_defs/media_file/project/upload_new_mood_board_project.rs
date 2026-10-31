use serde_derive::{Deserialize, Serialize};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const UPLOAD_NEW_MOOD_BOARD_PROJECT_URL_PATH: &str = "/v1/media_files/upload/project/mood_board/new";

/// Response for saving a new mood board project.
#[derive(Serialize, Deserialize)]
pub struct UploadNewMoodBoardProjectSuccessResponse {
  pub success: bool,

  /// Token for the newly created project media file. Use it to query the
  /// project and to save subsequent updates via the update endpoint.
  pub media_file_token: MediaFileToken,
}
