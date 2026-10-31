use std::path::{Path, PathBuf};

use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;

/// `<root>/cache` — regenerable derived data (thumbnails, previews). Safe to
/// delete wholesale; everything here rebuilds on demand.
#[derive(Clone)]
pub struct AppCacheDir {
  path: PathBuf,
}

impl DataSubdir for AppCacheDir {
  const DIRECTORY_NAME: &'static str = "cache";

  fn new_from<P: AsRef<Path>>(dir: P) -> Self {
    Self {
      path: dir.as_ref().to_path_buf(),
    }
  }

  fn path(&self) -> &Path {
    &self.path
  }
}

impl AppCacheDir {
  /// `cache/thumbnails/` — content-addressed thumbnails
  /// (`{blake3_hex}.jpg` static, `{blake3_hex}.webp` animated preview).
  /// Created on demand.
  pub fn get_or_create_thumbnails_dir(&self) -> anyhow::Result<PathBuf> {
    let dir = self.path.join("thumbnails");
    if !dir.exists() {
      std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
  }
}
