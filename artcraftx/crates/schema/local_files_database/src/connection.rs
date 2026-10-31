use std::path::Path;
use std::time::Duration;

use errors::AnyhowResult;
use log::{error, info};
use sqlx::migrate::MigrateError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{Pool, Sqlite, SqlitePool};

/// How long a writer waits for the lock before erroring. Thumbnail
/// generation writes in bursts; without this SQLite returns SQLITE_BUSY
/// immediately.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct LocalFilesDbConnection {
  pool: Pool<Sqlite>,
}

impl LocalFilesDbConnection {
  /// Open (creating if missing) and migrate the database.
  ///
  /// On migration failure the file is moved aside (`<name>.broken-<unix ms>`)
  /// and a fresh one created. Nothing here is irreplaceable — thumbnails
  /// regenerate and the upload ledger only costs a re-upload — but the
  /// ledger is worth keeping for a hand recovery, so it isn't deleted.
  pub async fn connect_and_migrate<P: AsRef<Path>>(database_file: P) -> AnyhowResult<Self> {
    match run_migrations(&database_file).await {
      Ok(pool) => return Ok(Self { pool }),
      Err(err) => {
        error!("Error running local_files SQLite migrations: {:?}", err);
      }
    }

    let database_file = database_file.as_ref();
    let unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let aside = database_file.with_extension(format!("broken-{unix_ms}"));
    info!("Moving local_files SQLite database aside to {:?} and recreating it", aside);

    if let Err(err) = std::fs::rename(database_file, &aside) {
      error!("Error moving local_files SQLite database file aside: {:?}", err);
    }
    // WAL/shm sidecars belong to the old file; a stale WAL must not replay
    // into the new database.
    for sidecar in ["-wal", "-shm"] {
      let mut path = database_file.as_os_str().to_owned();
      path.push(sidecar);
      let _ = std::fs::remove_file(path);
    }

    let pool = run_migrations(database_file).await?;

    Ok(Self { pool })
  }

  pub fn get_pool(&self) -> &Pool<Sqlite> {
    &self.pool
  }
}

async fn run_migrations<P: AsRef<Path>>(database_file: P) -> Result<SqlitePool, MigrateError> {
  let connection_options = SqliteConnectOptions::new()
      .filename(database_file)
      .create_if_missing(true)
      .journal_mode(SqliteJournalMode::Wal)
      .synchronous(SqliteSynchronous::Normal)
      .busy_timeout(BUSY_TIMEOUT);

  let pool = SqlitePool::connect_with(connection_options).await?;

  // Additive migrations against a stable filename; sqlx tracks which have
  // run. The SQL is compiled into the binary.
  sqlx::migrate!().run(&pool).await?;

  Ok(pool)
}
