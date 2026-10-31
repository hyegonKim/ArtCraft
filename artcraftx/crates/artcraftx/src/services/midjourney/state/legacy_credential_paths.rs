//! Legacy on-disk credential paths for Midjourney.
//!
//! TODO(artcraftx): retire once Midjourney moves to the unified TOML
//! credential store in `crate::credentials`.

use crate::state::data_dir::subdirectory::app_credentials_dir::AppCredentialsDir;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use std::path::PathBuf;

pub trait MidjourneyLegacyCredentialPaths {
  fn get_midjourney_state_path(&self) -> PathBuf;
}

impl MidjourneyLegacyCredentialPaths for AppCredentialsDir {
  fn get_midjourney_state_path(&self) -> PathBuf {
    self.path().join("midjourney_state.json")
  }
}
