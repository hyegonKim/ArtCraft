use crate::state::app_preferences::settings::preferred_download_directory::{PreferredDownloadDirectory, SystemDownloadDirectory};
use crate::state::app_preferences::settings::preferred_download_filename::PreferredDownloadFilename;
use serde_derive::{Deserialize, Serialize};

/// Where downloaded files go and what they're called.
///
/// Missing fields in an older preferences file fall back to the defaults.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppDownloadPreferences {
  /// The downloads directory to use when a user downloads a file.
  pub preferred_download_directory: PreferredDownloadDirectory,

  /// How downloaded generation files are named on disk.
  pub preferred_download_filename: PreferredDownloadFilename,
}

impl Default for AppDownloadPreferences {
  fn default() -> Self {
    Self {
      preferred_download_directory: PreferredDownloadDirectory::System(SystemDownloadDirectory::Downloads),
      preferred_download_filename: PreferredDownloadFilename::ArtcraftConvention,
    }
  }
}
