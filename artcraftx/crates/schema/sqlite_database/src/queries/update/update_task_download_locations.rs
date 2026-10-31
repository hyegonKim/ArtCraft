use crate::connection::TaskDbConnection;
use crate::error::SqliteTasksError;
use sqlite_identifiers::ids::task_id::TaskId;
use std::path::Path;

pub struct UpdateTaskDownloadLocationsArgs<'a> {
  pub db: &'a TaskDbConnection,
  pub task_id: &'a TaskId,
  /// The directory the results were saved into.
  pub directory_location: &'a Path,
  /// The first (or only) downloaded file.
  pub first_file_location: &'a Path,
}

/// Record where a completed task's results were downloaded. Paths are stored
/// as given (absolute), so later changes to the preferred download directory
/// don't affect them. Returns true if rows were updated.
pub async fn update_task_download_locations(
  args: UpdateTaskDownloadLocationsArgs<'_>,
) -> Result<bool, SqliteTasksError> {
  let task_id_temp = args.task_id.as_str();
  let directory_temp = args.directory_location.to_string_lossy().into_owned();
  let first_file_temp = args.first_file_location.to_string_lossy().into_owned();

  let query = sqlx::query!(r#"
    UPDATE tasks
    SET
      on_complete_directory_location = ?,
      on_complete_first_file_location = ?
    WHERE id = ?
  "#,
      directory_temp,
      first_file_temp,
      task_id_temp,
  );

  let res = query.execute(args.db.get_pool()).await?;
  Ok(res.rows_affected() > 0)
}
