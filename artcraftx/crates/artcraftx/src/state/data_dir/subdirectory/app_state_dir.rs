use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use crate::state::database::task_database_version::TASK_DATABASE_VERSION;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct AppStateDir {
  path: PathBuf,
}

impl DataSubdir for AppStateDir{
  const DIRECTORY_NAME: &'static str = "state";

  fn new_from<P: AsRef<Path>>(dir: P) -> Self {
    Self {
      path: dir.as_ref().to_path_buf(),
    }
  }

  fn path(&self) -> &Path {
    &self.path
  }
}

impl AppStateDir {
  pub fn get_tasks_sqlite_database_path(&self) -> PathBuf {
    // NB: This must be incremented on any change to the database schema (including comments).
    // The database houses ephemeral content and will be migrated automatically.
    self.path.join(format!("tasks_v{TASK_DATABASE_VERSION}.sqlite"))
  }

  /// The local_files content-hash index. Unlike the tasks database this
  /// keeps a STABLE filename: it migrates additively, and on migration
  /// failure it's deleted and rebuilt (every row is derivable).
  pub fn get_local_files_sqlite_database_path(&self) -> PathBuf {
    self.path.join("local_files.sqlite")
  }

  pub fn get_window_size_config_file(&self) -> PathBuf {
    self.path.join("window_size.json")
  }

  pub fn get_window_position_config_file(&self) -> PathBuf {
    self.path.join("window_position.json")
  }

  /// What each prompt box last had selected (see `state::promptbox`).
  pub fn get_promptbox_state_path(&self) -> PathBuf {
    self.path.join("promptbox_state.json")
  }
}
