//! Skip re-uploading bytes a provider already holds.
//!
//! The router uploads reference media to providers on the caller's behalf
//! (Higgsfield today). The app keeps a ledger of what each account has
//! already uploaded, keyed by content hash; it hands the router a
//! per-account view of that ledger through this trait, and the router asks
//! before every upload. The router has no idea how the ledger is stored.
//!
//! Entries are advisory: the router verifies a hit with the provider before
//! trusting it, and any failure falls back to a fresh upload that is then
//! recorded over the stale entry.

use async_trait::async_trait;

/// How the provider came to hold some content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetUploadOrigin {
  /// We uploaded the bytes; `service_id` is the provider's media id.
  Upload,
  /// The provider generated the file itself and we downloaded it;
  /// `service_id` is the job id, and the content can be referenced as a
  /// previous generation without uploading it (or re-running an IP check).
  GenerationResult,
}

impl AssetUploadOrigin {
  /// The stored form.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Upload => "upload",
      Self::GenerationResult => "generation_result",
    }
  }

  /// Unknown values read as plain uploads (the conservative choice: they get
  /// verified as media before reuse).
  pub fn parse(value: &str) -> Self {
    match value {
      "generation_result" => Self::GenerationResult,
      _ => Self::Upload,
    }
  }
}

/// A copy of some bytes the provider already holds, as the provider knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedAssetUpload {
  /// The provider's own id for the asset: a media id for an upload, a job
  /// id for a generation result.
  pub service_id: String,
  /// Where the provider serves it, when it publishes a URL.
  pub maybe_service_url: Option<String>,
  pub origin: AssetUploadOrigin,
}

impl CachedAssetUpload {
  /// Bytes we uploaded.
  pub fn uploaded(service_id: impl Into<String>, maybe_service_url: Option<String>) -> Self {
    Self { service_id: service_id.into(), maybe_service_url, origin: AssetUploadOrigin::Upload }
  }

  /// A generation the provider produced, by job id and result URL.
  pub fn generation_result(job_id: impl Into<String>, result_url: impl Into<String>) -> Self {
    Self { service_id: job_id.into(), maybe_service_url: Some(result_url.into()), origin: AssetUploadOrigin::GenerationResult }
  }
}

/// One account's upload history on one provider. Implementations must not
/// fail loudly: a lookup error reads as a miss, a record error is logged.
#[async_trait]
pub trait AssetUploadCache: Send + Sync {
  /// The upload previously made for these bytes, if any.
  async fn find_upload(&self, file_hash_blake3: &str) -> Option<CachedAssetUpload>;

  /// Remember a fresh upload (replacing any stale entry for the hash).
  async fn record_upload(&self, file_hash_blake3: &str, file_size_bytes: u64, upload: &CachedAssetUpload);

  /// Note that an upload was skipped thanks to a (verified) entry.
  async fn note_reused(&self, file_hash_blake3: &str);

  /// Drop an entry the provider no longer honours.
  async fn forget_upload(&self, file_hash_blake3: &str);
}
