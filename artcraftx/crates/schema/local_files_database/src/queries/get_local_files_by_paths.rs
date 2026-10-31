use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite};

use crate::connection::LocalFilesDbConnection;
use crate::error::LocalFilesDbError;
use crate::queries::local_file_record::LocalFileRecord;

/// SQLite's default bound-parameter ceiling; larger inputs are chunked.
const MAX_PATHS_PER_QUERY: usize = 900;

/// Batch lookup by path (one query per 900 paths). Paths with no row are
/// simply absent from the map.
pub async fn get_local_files_by_paths(
  db: &LocalFilesDbConnection,
  file_paths: &[String],
) -> Result<HashMap<String, LocalFileRecord>, LocalFilesDbError> {
  let mut records = HashMap::with_capacity(file_paths.len());

  for chunk in file_paths.chunks(MAX_PATHS_PER_QUERY) {
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new("SELECT * FROM local_files WHERE file_path IN (");
    let mut separated = query_builder.separated(", ");
    for file_path in chunk {
      separated.push_bind(file_path);
    }
    separated.push_unseparated(")");

    let chunk_records = query_builder
        .build_query_as::<LocalFileRecord>()
        .fetch_all(db.get_pool())
        .await?;
    for record in chunk_records {
      records.insert(record.file_path.clone(), record);
    }
  }

  Ok(records)
}
