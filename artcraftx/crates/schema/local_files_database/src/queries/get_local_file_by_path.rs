use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;
use crate::queries::local_file_record::LocalFileRecord;

pub async fn get_local_file_by_path(
  db: &LocalFilesDbConnection,
  file_path: &str,
) -> Result<Option<LocalFileRecord>, LocalFilesDbError> {
  let record = sqlx::query_as::<_, LocalFileRecord>(
    "SELECT * FROM local_files WHERE file_path = ?1",
  )
      .bind(file_path)
      .fetch_optional(db.get_pool())
      .await?;
  Ok(record)
}
