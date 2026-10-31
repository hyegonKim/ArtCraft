use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;

/// Forget a remote copy (it no longer exists, or failed verification).
/// Returns whether a row was deleted.
pub async fn delete_asset_upload(
  db: &LocalFilesDbConnection,
  service: &str,
  account_id: &str,
  file_hash_blake3: &str,
) -> Result<bool, LocalFilesDbError> {
  let result = sqlx::query(
    "DELETE FROM asset_uploads WHERE service = ?1 AND account_id = ?2 AND file_hash_blake3 = ?3",
  )
      .bind(service)
      .bind(account_id)
      .bind(file_hash_blake3)
      .execute(db.get_pool())
      .await?;
  Ok(result.rows_affected() > 0)
}
