use std::env;
use std::path::PathBuf;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

pub fn main() {
  println!("cargo:rerun-if-changed=../../../../_database/sql/artcraft_migrations");

  let db_path = env::var_os("OUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(env::temp_dir)
    .join("tasks.sqlite");

  if db_path.exists() {
    let _ = std::fs::remove_file(&db_path);
  }

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .expect("tokio runtime for sqlite_tasks build.rs");

  rt.block_on(async {
    let opts = SqliteConnectOptions::new()
      .filename(&db_path)
      .create_if_missing(true);
    let pool = SqlitePool::connect_with(opts)
      .await
      .expect("failed to create build-time tasks.sqlite");
    sqlx::migrate!("../../../../_database/sql/artcraft_migrations")
      .run(&pool)
      .await
      .expect("failed to run migrations on build-time tasks.sqlite");
    pool.close().await;
  });

  println!("cargo:rustc-env=DATABASE_URL=sqlite:{}", db_path.display());
}
