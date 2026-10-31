//! Let the caller watch reference-media uploads as they happen.
//!
//! The router decides per file whether to reuse a provider's existing copy
//! (see [`crate::api::asset_upload_cache`]) or upload it — and the user
//! experience differs: a reuse is instant, an upload with an IP check is a
//! long wait. The caller learns which one happened through this trait, at
//! the moment it happens, so it can show the right notice.

/// Observes upload decisions for one request. Implementations must be
/// cheap and must not fail; they only inform the UI.
pub trait AssetUploadObserver: Send + Sync {
  /// A file was not uploaded because the provider already holds a verified
  /// copy. `kind` is e.g. "image" / "video" / "audio".
  fn on_reused(&self, kind: &str, description: &str);

  /// A fresh upload is starting. `ip_check` means the provider will also run
  /// (and the router will wait for) its intellectual-property check.
  fn on_uploading(&self, kind: &str, byte_count: u64, ip_check: bool);
}
