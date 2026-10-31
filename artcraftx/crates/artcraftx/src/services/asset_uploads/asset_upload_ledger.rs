//! The upload-dedup ledger: which remote service already holds which local
//! content, per account.
//!
//! Before an upload, callers hash the bytes (memoized through the
//! local_files index for files on disk) and ask the ledger. A hit is
//! verified with the service before it is trusted; a miss, or a hit the
//! service no longer honours, means a fresh upload that is then recorded
//! over the old row. The ledger never makes an upload fail: every lookup
//! error reads as a miss and every write error is logged.
//!
//! Account identity:
//! - Higgsfield: the stored credential's id (sessions only come from stored
//!   credentials).
//! - ArtCraft: a fingerprint of the web session cookie
//!   ([`artcraft_account_id`]) — the app has no user id for a session, and a
//!   re-login simply starts a fresh (empty) history.
//!
//! The router gets a per-account view through [`AccountUploadCache`], which
//! implements its [`AssetUploadCache`] trait.

use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};

use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use artcraft_client::utils::api_host::ApiHost;
use async_trait::async_trait;
use core_types::enums::generation_source::GenerationSource;
use core_types::identifiers::credential_id::CredentialId;
use file_hashing::hash_bytes_blake3;
use local_files_database::queries::asset_upload_record::AssetUploadRecord;
use local_files_database::queries::delete_asset_upload::delete_asset_upload;
use local_files_database::queries::get_asset_upload::{get_asset_upload, GetAssetUploadArgs};
use local_files_database::queries::touch_asset_upload_reused::touch_asset_upload_reused;
use local_files_database::queries::upsert_asset_upload::{upsert_asset_upload, UpsertAssetUploadArgs};
use log::{info, warn};
use router::api::asset_upload_cache::{AssetUploadCache, AssetUploadOrigin, CachedAssetUpload};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::services::local_files::file_hash_service::hash_local_file_memoized;
use crate::state::database::local_files_database::LocalFilesDatabase;

/// The ArtCraft account key is derived from the session cookie; this many
/// hex characters of its BLAKE3 is plenty to tell sessions apart.
const ARTCRAFT_SESSION_FINGERPRINT_CHARS: usize = 32;

#[derive(Clone)]
pub struct AssetUploadLedger {
  database: LocalFilesDatabase,
}

/// The ledger narrowed to one service + account: what the router (and the
/// ArtCraft upload helpers) work with.
#[derive(Clone)]
pub struct AccountUploadCache {
  ledger: AssetUploadLedger,
  service: GenerationSource,
  account_id: String,
  /// Hashes whose upload this request skipped thanks to a ledger hit. If the
  /// provider then rejects the request over its media, these are the entries
  /// to drop before uploading afresh (see `forget_reused`).
  reused_this_request: Arc<Mutex<Vec<String>>>,
}

impl AssetUploadLedger {
  pub fn new(database: LocalFilesDatabase) -> Self {
    Self { database }
  }

  /// This account's uploads on `service`.
  pub fn for_account(&self, service: GenerationSource, account_id: impl Into<String>) -> AccountUploadCache {
    AccountUploadCache {
      ledger: self.clone(),
      service,
      account_id: account_id.into(),
      reused_this_request: Arc::new(Mutex::new(Vec::new())),
    }
  }

  /// A stored Higgsfield credential's uploads.
  pub fn for_higgsfield_credential(&self, credential_id: &CredentialId) -> AccountUploadCache {
    self.for_account(GenerationSource::Higgsfield, credential_id.as_str())
  }

  /// The uploads of the ArtCraft account behind a web session, if the
  /// session identifies one (anonymous uploads aren't tracked).
  pub fn for_artcraft_session(&self, maybe_creds: Option<&StorytellerCredentialSet>) -> Option<AccountUploadCache> {
    let account_id = artcraft_account_id(maybe_creds?)?;
    Some(self.for_account(GenerationSource::Artcraft, account_id))
  }

  /// BLAKE3 of a file on disk, memoized in the local_files index.
  pub async fn hash_file(&self, path: &Path) -> Result<String, String> {
    hash_local_file_memoized(&self.database, path).await
  }

  // ── Raw row access ──

  pub async fn find(&self, service: GenerationSource, account_id: &str, file_hash: &str) -> Option<AssetUploadRecord> {
    let result = get_asset_upload(GetAssetUploadArgs {
      db: self.database.get_connection(),
      service: service.to_str(),
      account_id,
      file_hash_blake3: file_hash,
    }).await;
    match result {
      Ok(record) => record,
      Err(err) => {
        warn!("asset_uploads lookup failed ({service}, {account_id}, {file_hash}): {err}");
        None
      }
    }
  }

  pub async fn record(&self, service: GenerationSource, account_id: &str, file_hash: &str, file_size_bytes: Option<u64>, upload: &CachedAssetUpload) {
    let result = upsert_asset_upload(UpsertAssetUploadArgs {
      db: self.database.get_connection(),
      service: service.to_str(),
      account_id,
      file_hash_blake3: file_hash,
      service_id: &upload.service_id,
      origin: upload.origin.as_str(),
      maybe_service_url: upload.maybe_service_url.as_deref(),
      maybe_file_size_bytes: file_size_bytes.map(|size| size as i64),
    }).await;
    match result {
      Ok(()) => info!("Recorded {service} upload {} for {account_id} (hash {file_hash})", upload.service_id),
      Err(err) => warn!("Could not record {service} upload {}: {err}", upload.service_id),
    }
  }

  pub async fn note_reused(&self, service: GenerationSource, account_id: &str, file_hash: &str) {
    if let Err(err) = touch_asset_upload_reused(self.database.get_connection(), service.to_str(), account_id, file_hash).await {
      warn!("Could not mark {service} upload reused (hash {file_hash}): {err}");
    }
  }

  pub async fn forget(&self, service: GenerationSource, account_id: &str, file_hash: &str) {
    if let Err(err) = delete_asset_upload(self.database.get_connection(), service.to_str(), account_id, file_hash).await {
      warn!("Could not forget {service} upload (hash {file_hash}): {err}");
    }
  }
}

impl AccountUploadCache {
  pub fn service(&self) -> GenerationSource {
    self.service
  }

  /// How many uploads this request has skipped so far via ledger hits.
  pub fn reused_count(&self) -> usize {
    self.reused_this_request.lock().map(|reused| reused.len()).unwrap_or(0)
  }

  /// Drop every ledger entry this request reused, so the next finalize
  /// uploads those files afresh. Returns how many were forgotten.
  pub async fn forget_reused(&self) -> usize {
    let hashes: Vec<String> = match self.reused_this_request.lock() {
      Ok(mut reused) => reused.drain(..).collect(),
      Err(_) => Vec::new(),
    };
    for hash in &hashes {
      self.ledger.forget(self.service, &self.account_id, hash).await;
    }
    hashes.len()
  }

  pub fn account_id(&self) -> &str {
    &self.account_id
  }

  /// Hash a local file (memoized through the local_files index).
  pub async fn hash_file(&self, path: &Path) -> Result<String, String> {
    self.ledger.hash_file(path).await
  }

  /// The ArtCraft media file for a local file, reusing this account's
  /// earlier upload when ArtCraft still has it, else running `upload` and
  /// recording its token. `upload` is only awaited on a miss.
  ///
  /// Hashing failures (unreadable file) skip the ledger and just upload.
  pub async fn artcraft_token_for_file<F, Fut, E>(&self, api_host: &ApiHost, path: &Path, upload: F) -> Result<MediaFileToken, E>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<MediaFileToken, E>>,
  {
    let maybe_hash = match self.hash_file(path).await {
      Ok(hash) => Some(hash),
      Err(message) => {
        warn!("Could not hash {} for the upload ledger ({message}); uploading without it", path.display());
        None
      }
    };

    if let Some(hash) = maybe_hash.as_deref() {
      if let Some(token) = self.verified_artcraft_token(api_host, hash).await {
        info!("Reusing ArtCraft media file {} for {} (already uploaded by this account)", token.as_str(), path.display());
        self.note_reused(hash).await;
        return Ok(token);
      }
    }

    let token = upload().await?;

    if let Some(hash) = maybe_hash.as_deref() {
      let file_size_bytes = std::fs::metadata(path).ok().map(|metadata| metadata.len());
      self.record_upload(hash, file_size_bytes.unwrap_or(0), &CachedAssetUpload::uploaded(token.as_str().to_string(), None)).await;
    }
    Ok(token)
  }

  /// Same as [`Self::artcraft_token_for_file`] for bytes already in memory.
  pub async fn artcraft_token_for_bytes<F, Fut, E>(&self, api_host: &ApiHost, bytes: &[u8], upload: F) -> Result<MediaFileToken, E>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<MediaFileToken, E>>,
  {
    let hash = hash_bytes_blake3(bytes);
    if let Some(token) = self.verified_artcraft_token(api_host, &hash).await {
      info!("Reusing ArtCraft media file {} for {} in-memory bytes (already uploaded by this account)", token.as_str(), bytes.len());
      self.note_reused(&hash).await;
      return Ok(token);
    }
    let token = upload().await?;
    self.record_upload(&hash, bytes.len() as u64, &CachedAssetUpload::uploaded(token.as_str().to_string(), None)).await;
    Ok(token)
  }

  /// The cached ArtCraft token for a hash, if ArtCraft still serves the
  /// media file. Anything else forgets the row and answers `None`.
  async fn verified_artcraft_token(&self, api_host: &ApiHost, file_hash: &str) -> Option<MediaFileToken> {
    let cached = self.find_upload(file_hash).await?;
    let token = MediaFileToken::new_from_str(&cached.service_id);
    match get_media_file(api_host, &token).await {
      Ok(_) => Some(token),
      Err(err) => {
        warn!("Cached ArtCraft media file {} could not be confirmed ({:?}); uploading again", token.as_str(), err);
        self.forget_upload(file_hash).await;
        None
      }
    }
  }
}

#[async_trait]
impl AssetUploadCache for AccountUploadCache {
  async fn find_upload(&self, file_hash_blake3: &str) -> Option<CachedAssetUpload> {
    self.ledger.find(self.service, &self.account_id, file_hash_blake3).await
        .map(|record| CachedAssetUpload {
          service_id: record.service_id,
          maybe_service_url: record.service_url,
          origin: AssetUploadOrigin::parse(&record.origin),
        })
  }

  async fn record_upload(&self, file_hash_blake3: &str, file_size_bytes: u64, upload: &CachedAssetUpload) {
    self.ledger.record(self.service, &self.account_id, file_hash_blake3, Some(file_size_bytes), upload).await
  }

  async fn note_reused(&self, file_hash_blake3: &str) {
    if let Ok(mut reused) = self.reused_this_request.lock() {
      reused.push(file_hash_blake3.to_string());
    }
    self.ledger.note_reused(self.service, &self.account_id, file_hash_blake3).await
  }

  async fn forget_upload(&self, file_hash_blake3: &str) {
    self.ledger.forget(self.service, &self.account_id, file_hash_blake3).await
  }
}

/// The ledger's account key for an ArtCraft web session: a fingerprint of
/// the session cookie. `None` without a session (anonymous).
pub fn artcraft_account_id(creds: &StorytellerCredentialSet) -> Option<String> {
  let session = creds.session.as_ref()?;
  let fingerprint = hash_bytes_blake3(session.as_bytes());
  Some(format!("artcraft_session_{}", &fingerprint[..ARTCRAFT_SESSION_FINGERPRINT_CHARS]))
}

#[cfg(test)]
mod tests {
  use super::*;
  use artcraft_client::credentials::storyteller_session_cookie::StorytellerSessionCookie;
  use crate::state::data_dir::app_data_root::AppDataRoot;

  #[test]
  fn artcraft_account_id_is_a_stable_session_fingerprint() {
    let mut creds = StorytellerCredentialSet::empty();
    assert!(artcraft_account_id(&creds).is_none());

    creds.session = Some(StorytellerSessionCookie::new("session-abc".to_string()));
    let id = artcraft_account_id(&creds).unwrap();
    assert!(id.starts_with("artcraft_session_"));
    assert_eq!(id.len(), "artcraft_session_".len() + ARTCRAFT_SESSION_FINGERPRINT_CHARS);
    assert_eq!(artcraft_account_id(&creds).unwrap(), id);
    assert!(!id.contains("session-abc"), "the raw cookie must not leak into the key");

    creds.session = Some(StorytellerSessionCookie::new("session-xyz".to_string()));
    assert_ne!(artcraft_account_id(&creds).unwrap(), id);
  }

  #[tokio::test]
  async fn account_cache_round_trips_through_the_router_trait() {
    let dir = tempfile::tempdir().unwrap();
    let root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let ledger = AssetUploadLedger::new(LocalFilesDatabase::connect(&root).await.unwrap());
    let cache = ledger.for_higgsfield_credential(&CredentialId::from_trusted("credential_test"));
    let other = ledger.for_higgsfield_credential(&CredentialId::from_trusted("credential_other"));

    assert!(cache.find_upload("hash1").await.is_none());
    let upload = CachedAssetUpload::uploaded("media-1", Some("https://cdn/x".into()));
    cache.record_upload("hash1", 10, &upload).await;
    assert_eq!(cache.find_upload("hash1").await, Some(upload));
    assert!(other.find_upload("hash1").await.is_none(), "accounts don't share uploads");

    cache.note_reused("hash1").await;
    assert!(ledger.find(GenerationSource::Higgsfield, "credential_test", "hash1").await.unwrap().last_reused_at.is_some());

    cache.forget_upload("hash1").await;
    assert!(cache.find_upload("hash1").await.is_none());
  }

  #[tokio::test]
  async fn generation_results_keep_their_origin() {
    let dir = tempfile::tempdir().unwrap();
    let root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let ledger = AssetUploadLedger::new(LocalFilesDatabase::connect(&root).await.unwrap());
    let cache = ledger.for_higgsfield_credential(&CredentialId::from_trusted("credential_test"));
    let result = CachedAssetUpload::generation_result("job-1", "https://cdn/result.png");
    cache.record_upload("hash-result", 10, &result).await;
    assert_eq!(cache.find_upload("hash-result").await, Some(result));
    let plain = CachedAssetUpload::uploaded("media-1", Some("https://cdn/x".into()));
    cache.record_upload("hash-upload", 10, &plain).await;
    assert_eq!(cache.find_upload("hash-upload").await.unwrap().origin, AssetUploadOrigin::Upload);
  }

  #[tokio::test]
  async fn forget_reused_drops_only_what_this_request_reused() {
    let dir = tempfile::tempdir().unwrap();
    let root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let ledger = AssetUploadLedger::new(LocalFilesDatabase::connect(&root).await.unwrap());
    let cache = ledger.for_higgsfield_credential(&CredentialId::from_trusted("credential_test"));
    let upload = |id: &str| CachedAssetUpload::uploaded(id, Some("https://cdn/x".into()));
    cache.record_upload("reused", 10, &upload("media-reused")).await;
    cache.record_upload("fresh", 10, &upload("media-fresh")).await;
    assert_eq!(cache.reused_count(), 0);

    cache.note_reused("reused").await;
    assert_eq!(cache.reused_count(), 1);

    assert_eq!(cache.forget_reused().await, 1);
    assert_eq!(cache.reused_count(), 0, "cleared for the retry");
    assert!(cache.find_upload("reused").await.is_none(), "the reused entry is gone");
    assert!(cache.find_upload("fresh").await.is_some(), "an entry that wasn't reused survives");
    assert_eq!(cache.forget_reused().await, 0, "nothing left to forget");
  }

  #[tokio::test]
  async fn artcraft_file_helper_uploads_on_miss_and_records() {
    let dir = tempfile::tempdir().unwrap();
    let root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let ledger = AssetUploadLedger::new(LocalFilesDatabase::connect(&root).await.unwrap());
    let cache = ledger.for_account(GenerationSource::Artcraft, "artcraft_session_test");
    let file = dir.path().join("ref.png");
    std::fs::write(&file, b"\x89PNG\r\n\x1a\nfake").unwrap();

    let token = cache.artcraft_token_for_file::<_, _, ()>(&ApiHost::Storyteller, &file, || async {
      Ok(MediaFileToken::new_from_str("m_fresh"))
    }).await.unwrap();
    assert_eq!(token.as_str(), "m_fresh");

    let hash = cache.hash_file(&file).await.unwrap();
    let recorded = cache.find_upload(&hash).await.unwrap();
    assert_eq!(recorded.service_id, "m_fresh");
    assert_eq!(ledger.find(GenerationSource::Artcraft, "artcraft_session_test", &hash).await.unwrap().file_size_bytes, Some(12));
  }
}
