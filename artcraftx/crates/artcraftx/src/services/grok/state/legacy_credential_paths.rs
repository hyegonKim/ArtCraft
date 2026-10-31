//! Legacy on-disk credential paths for Grok.
//!
//! TODO(artcraftx): retire once Grok moves to the unified TOML credential
//! store in `crate::credentials`.

use crate::state::data_dir::subdirectory::app_credentials_dir::AppCredentialsDir;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use std::path::PathBuf;

pub trait GrokLegacyCredentialPaths {
  fn get_grok_state_path(&self) -> PathBuf;
  fn get_grok_cookies_path(&self) -> PathBuf;
}

impl GrokLegacyCredentialPaths for AppCredentialsDir {
  fn get_grok_state_path(&self) -> PathBuf {
    self.path().join("grok_state.json")
  }

  fn get_grok_cookies_path(&self) -> PathBuf {
    self.path().join("grok_cookies.txt")
  }
}
