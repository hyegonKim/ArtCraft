use crate::state::data_dir::app_data_root::AppDataRoot;
use errors::AnyhowResult;
use local_files_database::connection::LocalFilesDbConnection;

/// The content-hash index of local files (thumbnail accounting today; the
/// upload-dedup ledger later). Deliberately a separate SQLite file from the
/// tasks database: it's a rebuildable index with its own write bursts and
/// lock domain.
#[derive(Clone)]
pub struct LocalFilesDatabase {
  connection: LocalFilesDbConnection,
}

impl LocalFilesDatabase {
  pub async fn connect(root: &AppDataRoot) -> AnyhowResult<Self> {
    let path = root.state_dir().get_local_files_sqlite_database_path();
    let connection = LocalFilesDbConnection::connect_and_migrate(path).await?;
    Ok(Self { connection })
  }

  pub fn get_connection(&self) -> &LocalFilesDbConnection {
    &self.connection
  }
}
