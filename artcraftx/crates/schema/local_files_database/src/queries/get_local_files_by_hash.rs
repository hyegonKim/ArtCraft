use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;
use crate::queries::local_file_record::LocalFileRecord;

/// Every known path with this content hash (dedup lookups; the future
/// upload-dedup ledger keys on this).
pub async fn get_local_files_by_hash(
  db: &LocalFilesDbConnection,
  file_hash_blake3: &str,
) -> Result<Vec<LocalFileRecord>, LocalFilesDbError> {
  let records = sqlx::query_as::<_, LocalFileRecord>(
    "SELECT * FROM local_files WHERE file_hash_blake3 = ?1 ORDER BY file_path",
  )
      .bind(file_hash_blake3)
      .fetch_all(db.get_pool())
      .await?;
  Ok(records)
}
