//! Integration tests against a throwaway migrated database in a tempdir.

use local_files_database::connection::LocalFilesDbConnection;
use local_files_database::queries::get_local_file_by_path::get_local_file_by_path;
use local_files_database::queries::get_local_files_by_hash::get_local_files_by_hash;
use local_files_database::queries::get_local_files_by_paths::get_local_files_by_paths;
use local_files_database::queries::list_local_files_needing_thumbnails::list_local_files_needing_thumbnails;
use local_files_database::queries::set_thumbnail_result::{set_thumbnail_result, SetThumbnailResultArgs};
use local_files_database::queries::upsert_local_file::{upsert_local_file, UpsertLocalFileArgs};
use local_files_database::thumbnail_state::ThumbnailState;

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const GENERATOR_VERSION: i64 = 2;
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[tokio::test]
async fn insert_read_and_stat_matching() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  assert!(get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().is_none());

  upsert(&db, "/a/video.mp4", 1000, 111, HASH_A).await;
  let record = get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().unwrap();
  assert_eq!(record.file_hash_blake3, HASH_A);
  assert_eq!(record.file_type.as_deref(), Some("mp4"));
  assert!(!record.has_thumbnail);
  assert_eq!(record.thumbnail_state(), ThumbnailState::Pending);
  assert_eq!(record.thumbnail_attempts, 0);
  assert!(record.matches_stat(1000, 111));
  assert!(!record.matches_stat(1000, 222));
  assert!(!record.matches_stat(999, 111));
}

#[tokio::test]
async fn thumbnail_result_survives_same_hash_upsert_but_resets_on_new_hash() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  upsert(&db, "/a/video.mp4", 1000, 111, HASH_A).await;
  assert!(record_generated(&db, "/a/video.mp4", true).await);
  assert!(!record_generated(&db, "/missing", true).await);

  // Same content re-upserted (e.g. mtime refreshed): bookkeeping survives.
  upsert(&db, "/a/video.mp4", 1000, 222, HASH_A).await;
  let record = get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().unwrap();
  assert!(record.has_thumbnail);
  assert!(record.has_animated_preview);
  assert_eq!(record.thumbnail_state(), ThumbnailState::Generated);
  assert_eq!(record.thumbnail_attempts, 1);
  assert_eq!(record.thumbnail_generator_version, GENERATOR_VERSION);
  assert_eq!(record.thumbnail_jpg_path.as_deref(), Some("/cache/a.jpg"));
  assert_eq!(record.animated_preview_webp_path.as_deref(), Some("/cache/a.webp"));
  assert!(record.thumbnail_updated_at.is_some());
  assert_eq!(record.file_mtime_ms, 222);

  // File overwritten in place with new content: everything resets.
  upsert(&db, "/a/video.mp4", 2000, 333, HASH_B).await;
  let record = get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().unwrap();
  assert!(!record.has_thumbnail);
  assert!(!record.has_animated_preview);
  assert_eq!(record.thumbnail_state(), ThumbnailState::Pending);
  assert_eq!(record.thumbnail_attempts, 0);
  assert!(record.thumbnail_jpg_path.is_none());
  assert!(record.animated_preview_webp_path.is_none());
  assert_eq!(record.thumbnail_generator_version, 0);
  assert_eq!(record.file_hash_blake3, HASH_B);
}

#[tokio::test]
async fn failures_count_attempts_and_keep_the_error() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;
  upsert(&db, "/a/video.mp4", 1000, 111, HASH_A).await;

  set_thumbnail_result(SetThumbnailResultArgs {
    db: &db,
    file_path: "/a/video.mp4",
    state: ThumbnailState::Failed,
    generator_version: GENERATOR_VERSION,
    maybe_thumbnail_jpg_path: None,
    maybe_animated_preview_webp_path: None,
    maybe_error: Some("decoder exploded"),
  }).await.unwrap();

  let record = get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().unwrap();
  assert_eq!(record.thumbnail_state(), ThumbnailState::Failed);
  assert_eq!(record.thumbnail_attempts, 1);
  assert_eq!(record.thumbnail_last_error.as_deref(), Some("decoder exploded"));
  assert!(!record.has_thumbnail);

  // A later success clears the error.
  assert!(record_generated(&db, "/a/video.mp4", false).await);
  let record = get_local_file_by_path(&db, "/a/video.mp4").await.unwrap().unwrap();
  assert_eq!(record.thumbnail_attempts, 2);
  assert!(record.thumbnail_last_error.is_none());
  assert!(record.has_thumbnail);
  assert!(!record.has_animated_preview);
}

#[tokio::test]
async fn worker_scan_lists_pending_and_retryable_failures_only() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  upsert(&db, "/pending.mp4", 1, 1, HASH_A).await;
  upsert(&db, "/generated.mp4", 1, 1, HASH_A).await;
  upsert(&db, "/unsupported.mp4", 1, 1, HASH_A).await;
  upsert(&db, "/failed_once.mp4", 1, 1, HASH_A).await;
  upsert(&db, "/failed_thrice.mp4", 1, 1, HASH_A).await;

  record_generated(&db, "/generated.mp4", true).await;
  record_state(&db, "/unsupported.mp4", ThumbnailState::Unsupported).await;
  record_state(&db, "/failed_once.mp4", ThumbnailState::Failed).await;
  for _ in 0..3 {
    record_state(&db, "/failed_thrice.mp4", ThumbnailState::Failed).await;
  }

  let mut paths: Vec<String> = list_local_files_needing_thumbnails(&db, 3, GENERATOR_VERSION, 100).await.unwrap()
      .into_iter()
      .map(|record| record.file_path)
      .collect();
  paths.sort();
  assert_eq!(paths, vec!["/failed_once.mp4".to_string(), "/pending.mp4".to_string()]);

  let limited = list_local_files_needing_thumbnails(&db, 3, GENERATOR_VERSION, 1).await.unwrap();
  assert_eq!(limited.len(), 1);

  // A newer generator makes every generated row owed again.
  let mut paths: Vec<String> = list_local_files_needing_thumbnails(&db, 3, GENERATOR_VERSION + 1, 100).await.unwrap()
      .into_iter()
      .map(|record| record.file_path)
      .collect();
  paths.sort();
  assert_eq!(paths, vec!["/failed_once.mp4".to_string(), "/generated.mp4".to_string(), "/pending.mp4".to_string()]);
}

#[tokio::test]
async fn hash_lookup_finds_duplicate_paths() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  upsert(&db, "/a/video.mp4", 1000, 111, HASH_A).await;
  upsert(&db, "/b/copy.mp4", 1000, 999, HASH_A).await;
  upsert(&db, "/c/other.mp4", 500, 111, HASH_B).await;

  let matches = get_local_files_by_hash(&db, HASH_A).await.unwrap();
  assert_eq!(matches.len(), 2);
  assert_eq!(matches[0].file_path, "/a/video.mp4");
  assert_eq!(matches[1].file_path, "/b/copy.mp4");
}

#[tokio::test]
async fn batch_path_lookup_skips_unknown_paths() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  upsert(&db, "/a/video.mp4", 1000, 111, HASH_A).await;
  upsert(&db, "/b/copy.mp4", 1000, 999, HASH_A).await;

  let empty = get_local_files_by_paths(&db, &[]).await.unwrap();
  assert!(empty.is_empty());

  let paths = vec!["/a/video.mp4".to_string(), "/nope.mp4".to_string(), "/b/copy.mp4".to_string()];
  let found = get_local_files_by_paths(&db, &paths).await.unwrap();
  assert_eq!(found.len(), 2);
  assert!(found.contains_key("/a/video.mp4"));
  assert!(found.contains_key("/b/copy.mp4"));
  assert!(!found.contains_key("/nope.mp4"));
}

// ── Helpers ──

async fn test_db(dir: &tempfile::TempDir) -> LocalFilesDbConnection {
  LocalFilesDbConnection::connect_and_migrate(dir.path().join("local_files.sqlite"))
      .await
      .expect("connect and migrate")
}

async fn upsert(db: &LocalFilesDbConnection, path: &str, size: i64, mtime: i64, hash: &str) {
  upsert_local_file(UpsertLocalFileArgs {
    db,
    file_path: path,
    maybe_file_type: Some("mp4"),
    file_size_bytes: size,
    file_mtime_ms: mtime,
    file_hash_blake3: hash,
  })
      .await
      .expect("upsert");
}

async fn record_generated(db: &LocalFilesDbConnection, path: &str, with_animated: bool) -> bool {
  set_thumbnail_result(SetThumbnailResultArgs {
    db,
    file_path: path,
    state: ThumbnailState::Generated,
    generator_version: GENERATOR_VERSION,
    maybe_thumbnail_jpg_path: Some("/cache/a.jpg"),
    maybe_animated_preview_webp_path: with_animated.then_some("/cache/a.webp"),
    maybe_error: None,
  })
      .await
      .expect("set result")
}

async fn record_state(db: &LocalFilesDbConnection, path: &str, state: ThumbnailState) {
  set_thumbnail_result(SetThumbnailResultArgs {
    db,
    file_path: path,
    state,
    generator_version: GENERATOR_VERSION,
    maybe_thumbnail_jpg_path: None,
    maybe_animated_preview_webp_path: None,
    maybe_error: Some("nope"),
  })
      .await
      .expect("set result");
}
