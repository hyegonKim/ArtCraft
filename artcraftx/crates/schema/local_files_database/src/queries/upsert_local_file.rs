use chrono::Utc;

use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;

pub struct UpsertLocalFileArgs<'a> {
  pub db: &'a LocalFilesDbConnection,
  pub file_path: &'a str,
  pub maybe_file_type: Option<&'a str>,
  pub file_size_bytes: i64,
  pub file_mtime_ms: i64,
  pub file_hash_blake3: &'a str,
}

/// Insert or refresh the row for a path. A changed hash (file overwritten
/// in place) resets all thumbnail bookkeeping to `pending` — the cache
/// entries for the old hash no longer describe this path's content.
pub async fn upsert_local_file(args: UpsertLocalFileArgs<'_>) -> Result<(), LocalFilesDbError> {
  let now = Utc::now();
  sqlx::query(
    r#"
    INSERT INTO local_files
      (file_path, file_type, file_size_bytes, file_mtime_ms, file_hash_blake3,
       has_thumbnail, has_animated_preview, thumbnail_state, thumbnail_attempts,
       created_at, updated_at)
    VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 'pending', 0, ?6, ?6)
    ON CONFLICT(file_path) DO UPDATE SET
      file_type = excluded.file_type,
      file_size_bytes = excluded.file_size_bytes,
      file_mtime_ms = excluded.file_mtime_ms,
      file_hash_blake3 = excluded.file_hash_blake3,
      has_thumbnail = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.has_thumbnail ELSE 0 END,
      has_animated_preview = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.has_animated_preview ELSE 0 END,
      thumbnail_state = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.thumbnail_state ELSE 'pending' END,
      thumbnail_attempts = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.thumbnail_attempts ELSE 0 END,
      thumbnail_last_error = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.thumbnail_last_error ELSE NULL END,
      thumbnail_jpg_path = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.thumbnail_jpg_path ELSE NULL END,
      animated_preview_webp_path = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.animated_preview_webp_path ELSE NULL END,
      thumbnail_generator_version = CASE
        WHEN local_files.file_hash_blake3 = excluded.file_hash_blake3
        THEN local_files.thumbnail_generator_version ELSE 0 END,
      updated_at = excluded.updated_at
    "#,
  )
      .bind(args.file_path)
      .bind(args.maybe_file_type)
      .bind(args.file_size_bytes)
      .bind(args.file_mtime_ms)
      .bind(args.file_hash_blake3)
      .bind(now)
      .execute(args.db.get_pool())
      .await?;
  Ok(())
}
