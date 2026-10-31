//! Legacy on-disk credential paths for Storyteller / Artcraft.
//!
//! TODO(artcraftx): retire once Storyteller moves to the unified TOML
//! credential store in `crate::credentials`.

use crate::state::data_dir::subdirectory::app_credentials_dir::AppCredentialsDir;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use std::path::PathBuf;

pub trait StorytellerLegacyCredentialPaths {
  fn get_storyteller_avt_cookie_file_path(&self) -> PathBuf;
  fn get_storyteller_session_cookie_file_path(&self) -> PathBuf;
}

impl StorytellerLegacyCredentialPaths for AppCredentialsDir {
  fn get_storyteller_avt_cookie_file_path(&self) -> PathBuf {
    self.path().join("artcraft_avt.txt")
  }

  fn get_storyteller_session_cookie_file_path(&self) -> PathBuf {
    self.path().join("artcraft_session.txt")
  }
}
