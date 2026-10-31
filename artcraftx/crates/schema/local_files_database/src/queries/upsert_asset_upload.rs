use chrono::Utc;

use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;

pub struct UpsertAssetUploadArgs<'a> {
  pub db: &'a LocalFilesDbConnection,
  pub service: &'a str,
  pub account_id: &'a str,
  pub file_hash_blake3: &'a str,
  pub service_id: &'a str,
  /// `upload` or `generation_result`.
  pub origin: &'a str,
  pub maybe_service_url: Option<&'a str>,
  pub maybe_file_size_bytes: Option<i64>,
}

/// Record (or replace) the remote copy of some content. A re-upload of the
/// same content overwrites the old service id and resets the reuse time.
pub async fn upsert_asset_upload(args: UpsertAssetUploadArgs<'_>) -> Result<(), LocalFilesDbError> {
  sqlx::query(
    r#"
    INSERT INTO asset_uploads
      (service, account_id, file_hash_blake3, service_id, origin, service_url, file_size_bytes, uploaded_at, last_reused_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)
    ON CONFLICT(service, account_id, file_hash_blake3) DO UPDATE SET
      service_id = excluded.service_id,
      origin = excluded.origin,
      service_url = excluded.service_url,
      file_size_bytes = excluded.file_size_bytes,
      uploaded_at = excluded.uploaded_at,
      last_reused_at = NULL
    "#,
  )
      .bind(args.service)
      .bind(args.account_id)
      .bind(args.file_hash_blake3)
      .bind(args.service_id)
      .bind(args.origin)
      .bind(args.maybe_service_url)
      .bind(args.maybe_file_size_bytes)
      .bind(Utc::now())
      .execute(args.db.get_pool())
      .await?;
  Ok(())
}
