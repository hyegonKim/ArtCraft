use chrono::Utc;

use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;

/// Note that a caller skipped an upload thanks to this row. Returns whether
/// the row existed.
pub async fn touch_asset_upload_reused(
  db: &LocalFilesDbConnection,
  service: &str,
  account_id: &str,
  file_hash_blake3: &str,
) -> Result<bool, LocalFilesDbError> {
  let result = sqlx::query(
    r#"
    UPDATE asset_uploads SET last_reused_at = ?4
    WHERE service = ?1 AND account_id = ?2 AND file_hash_blake3 = ?3
    "#,
  )
      .bind(service)
      .bind(account_id)
      .bind(file_hash_blake3)
      .bind(Utc::now())
      .execute(db.get_pool())
      .await?;
  Ok(result.rows_affected() > 0)
}
