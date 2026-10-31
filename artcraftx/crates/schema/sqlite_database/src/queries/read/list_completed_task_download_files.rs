use crate::connection::TaskDbConnection;
use crate::error::SqliteTasksError;
use sqlite_identifiers::enums::task_status::TaskStatus;

/// The first downloaded file of the most recently completed tasks (newest
/// first). Feeds the thumbnail worker's periodic scan: anything here without
/// a generated thumbnail is work.
///
/// NB: A runtime query on purpose — it needs no compile-time schema and
/// avoids regenerating the `.sqlx` offline data for a read-only scan.
pub async fn list_completed_task_download_files(
  db: &TaskDbConnection,
  limit: i64,
) -> Result<Vec<String>, SqliteTasksError> {
  let rows: Vec<(String,)> = sqlx::query_as(
    r#"
    SELECT on_complete_first_file_location
    FROM tasks
    WHERE task_status = ?1
      AND on_complete_first_file_location IS NOT NULL
    ORDER BY completed_at DESC
    LIMIT ?2
    "#,
  )
      .bind(TaskStatus::CompleteSuccess.to_str())
      .bind(limit)
      .fetch_all(db.get_pool())
      .await?;
  Ok(rows.into_iter().map(|(path,)| path).collect())
}
