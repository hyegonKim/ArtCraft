//! Integration tests for the upload-dedup ledger, against a throwaway
//! migrated database in a tempdir.

use local_files_database::connection::LocalFilesDbConnection;
use local_files_database::queries::delete_asset_upload::delete_asset_upload;
use local_files_database::queries::get_asset_upload::{get_asset_upload, GetAssetUploadArgs};
use local_files_database::queries::touch_asset_upload_reused::touch_asset_upload_reused;
use local_files_database::queries::upsert_asset_upload::{upsert_asset_upload, UpsertAssetUploadArgs};

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn record_lookup_and_reuse() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  assert!(lookup(&db, "higgsfield", "credential_1", HASH_A).await.is_none());

  record(&db, "higgsfield", "credential_1", HASH_A, "media-1", Some("https://cdn.higgsfield.ai/x.png")).await;
  let row = lookup(&db, "higgsfield", "credential_1", HASH_A).await.unwrap();
  assert_eq!(row.service_id, "media-1");
  assert_eq!(row.service_url.as_deref(), Some("https://cdn.higgsfield.ai/x.png"));
  assert_eq!(row.file_size_bytes, Some(1234));
  assert!(row.last_reused_at.is_none());

  assert!(touch_asset_upload_reused(&db, "higgsfield", "credential_1", HASH_A).await.unwrap());
  assert!(lookup(&db, "higgsfield", "credential_1", HASH_A).await.unwrap().last_reused_at.is_some());
  assert!(!touch_asset_upload_reused(&db, "higgsfield", "nobody", HASH_A).await.unwrap());
}

#[tokio::test]
async fn accounts_and_services_are_isolated() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  record(&db, "higgsfield", "credential_1", HASH_A, "media-1", None).await;
  record(&db, "higgsfield", "credential_2", HASH_A, "media-2", None).await;
  record(&db, "artcraft", "credential_1", HASH_A, "m_token", None).await;

  assert_eq!(lookup(&db, "higgsfield", "credential_1", HASH_A).await.unwrap().service_id, "media-1");
  assert_eq!(lookup(&db, "higgsfield", "credential_2", HASH_A).await.unwrap().service_id, "media-2");
  assert_eq!(lookup(&db, "artcraft", "credential_1", HASH_A).await.unwrap().service_id, "m_token");
  assert!(lookup(&db, "artcraft", "credential_2", HASH_A).await.is_none());
}

#[tokio::test]
async fn reupload_replaces_and_delete_forgets() {
  let dir = tempfile::tempdir().unwrap();
  let db = test_db(&dir).await;

  record(&db, "higgsfield", "credential_1", HASH_A, "media-old", Some("https://old")).await;
  touch_asset_upload_reused(&db, "higgsfield", "credential_1", HASH_A).await.unwrap();

  record(&db, "higgsfield", "credential_1", HASH_A, "media-new", None).await;
  let row = lookup(&db, "higgsfield", "credential_1", HASH_A).await.unwrap();
  assert_eq!(row.service_id, "media-new");
  assert!(row.service_url.is_none());
  assert!(row.last_reused_at.is_none(), "a fresh upload resets the reuse marker");

  assert!(delete_asset_upload(&db, "higgsfield", "credential_1", HASH_A).await.unwrap());
  assert!(!delete_asset_upload(&db, "higgsfield", "credential_1", HASH_A).await.unwrap());
  assert!(lookup(&db, "higgsfield", "credential_1", HASH_A).await.is_none());
}

// ── Helpers ──

async fn test_db(dir: &tempfile::TempDir) -> LocalFilesDbConnection {
  LocalFilesDbConnection::connect_and_migrate(dir.path().join("local_files.sqlite"))
      .await
      .expect("connect and migrate")
}

async fn record(db: &LocalFilesDbConnection, service: &str, account_id: &str, hash: &str, service_id: &str, url: Option<&str>) {
  upsert_asset_upload(UpsertAssetUploadArgs {
    db,
    service,
    account_id,
    file_hash_blake3: hash,
    service_id,
    origin: "upload",
    maybe_service_url: url,
    maybe_file_size_bytes: Some(1234),
  }).await.expect("upsert");
}

async fn lookup(db: &LocalFilesDbConnection, service: &str, account_id: &str, hash: &str) -> Option<local_files_database::queries::asset_upload_record::AssetUploadRecord> {
  get_asset_upload(GetAssetUploadArgs { db, service, account_id, file_hash_blake3: hash }).await.expect("get")
}
