use chrono::{DateTime, Utc};

use crate::thumbnail_state::ThumbnailState;

/// One row of `local_files`.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct LocalFileRecord {
  pub file_path: String,
  pub file_type: Option<String>,
  pub file_size_bytes: i64,
  pub file_mtime_ms: i64,
  pub file_hash_blake3: String,
  pub has_thumbnail: bool,
  pub has_animated_preview: bool,

  /// Raw `thumbnail_state` column; read it through [`Self::thumbnail_state`].
  #[sqlx(rename = "thumbnail_state")]
  pub thumbnail_state_raw: String,
  pub thumbnail_attempts: i64,
  pub thumbnail_last_error: Option<String>,
  pub thumbnail_jpg_path: Option<String>,
  pub animated_preview_webp_path: Option<String>,
  pub thumbnail_updated_at: Option<DateTime<Utc>>,
  /// Which generator version produced the cache entries (0 = none yet).
  pub thumbnail_generator_version: i64,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

impl LocalFileRecord {
  /// Whether the cached hash is still current for a file with this stat.
  pub fn matches_stat(&self, file_size_bytes: i64, file_mtime_ms: i64) -> bool {
    self.file_size_bytes == file_size_bytes && self.file_mtime_ms == file_mtime_ms
  }

  pub fn thumbnail_state(&self) -> ThumbnailState {
    ThumbnailState::from_str_lenient(&self.thumbnail_state_raw)
  }
}
