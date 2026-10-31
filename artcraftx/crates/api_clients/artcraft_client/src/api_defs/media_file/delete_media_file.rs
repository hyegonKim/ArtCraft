use serde_derive::{Deserialize, Serialize};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

#[derive(Serialize, Deserialize)]
pub struct DeleteMediaFileRequest {
  pub set_delete: bool,
  
  /// NB: this is only to disambiguate when a user is both a mod and an author.
  pub as_mod: Option<bool>,
}

/// For the URL PathInfo
#[derive(Serialize, Deserialize)]
pub struct DeleteMediaFilePathInfo {
  pub token: MediaFileToken,
}


