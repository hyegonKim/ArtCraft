//! Reference media for first-party Higgsfield: each source — an ArtCraft
//! media token, a public URL, a local file, or in-memory bytes — is turned
//! into bytes and uploaded through the session, and the resulting
//! [`MediaInput`] is what generation requests reference. Shared by the image
//! and video Higgsfield providers.
//!
//! Only ArtCraft media tokens touch the ArtCraft cloud (they resolve through
//! the token→URL map); local files and bytes go straight to Higgsfield.
//!
//! With an [`AssetUploadCache`] in hand, bytes the account already uploaded
//! are reused by id instead: the cached id is checked against Higgsfield's
//! media status endpoint first, and any doubt means a fresh upload.

use std::collections::HashMap;

use file_hashing::hash_bytes_blake3;
use higgsfield_client::endpoints::media::get_media_status::MediaStatusFamily;
use higgsfield_client::session::higgsfield_session::HiggsfieldSession;
use higgsfield_client::session::upload_media::ReferenceMediaFile;
use higgsfield_client::session::upload_source_guard::check_upload_source_url;
use higgsfield_client::types::ids::{JobId, MediaId};
use higgsfield_client::types::media_input::MediaInput;
use higgsfield_client::types::media_mime_type::MediaMimeType;
use log::{info, warn};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::asset_upload_observer::AssetUploadObserver;
use crate::api::asset_upload_cache::{AssetUploadCache, AssetUploadOrigin, CachedAssetUpload};
use crate::api::audio_list_ref::AudioListRef;
use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::video_list_ref::VideoListRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::utils::media_source_ref::{
  extension_from_name_hint, resolve_media_source_bytes, MediaSourceRef, ResolvedMediaBytes,
};

pub use crate::utils::media_source_ref::{resolve_media_token, resolve_media_tokens};

/// What a reference file is expected to be. Picks the upload endpoint family
/// and the fallback MIME type when the bytes and name don't say.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HiggsfieldMediaKind {
  Image,
  Video,
  Audio,
}

impl HiggsfieldMediaKind {
  /// Lower-case name for notices ("image" / "video" / "audio").
  pub fn label(self) -> &'static str {
    match self {
      Self::Image => "image",
      Self::Video => "video",
      Self::Audio => "audio",
    }
  }

  /// The status endpoint family for verifying a previous upload. Audio has
  /// none, so cached audio ids are trusted as-is.
  fn status_family(self) -> Option<MediaStatusFamily> {
    match self {
      Self::Image => Some(MediaStatusFamily::Image),
      Self::Video => Some(MediaStatusFamily::Video),
      Self::Audio => None,
    }
  }

  fn media_input(self, id: MediaId, url: String) -> MediaInput {
    match self {
      Self::Image => MediaInput::uploaded(id, url),
      Self::Video => MediaInput::uploaded_video(id, url),
      Self::Audio => MediaInput::uploaded_audio(id, url),
    }
  }

  /// A previous Higgsfield generation used as this kind of reference. Audio
  /// is never generated, so it has no job form.
  fn job_input(self, job_id: MediaId, url: String) -> Option<MediaInput> {
    match self {
      Self::Image => Some(MediaInput::from_image_job(job_id, url)),
      Self::Video => Some(MediaInput::from_video_job(job_id, url)),
      Self::Audio => None,
    }
  }

  fn fallback_mime_type(self) -> MediaMimeType {
    match self {
      Self::Image => MediaMimeType::ImageJpeg,
      Self::Video => MediaMimeType::VideoMp4,
      Self::Audio => MediaMimeType::AudioMpeg,
    }
  }

  fn accepts(self, mime_type: &MediaMimeType) -> bool {
    match self {
      Self::Image => mime_type.is_image(),
      Self::Video => mime_type.is_video(),
      Self::Audio => mime_type.is_audio(),
    }
  }
}

/// A list of reference sources, in upload order.
#[derive(Clone, Debug)]
pub struct HiggsfieldMediaSources(pub Vec<MediaSourceRef>);

impl From<ImageListRef> for HiggsfieldMediaSources {
  fn from(list: ImageListRef) -> Self {
    Self(MediaSourceRef::list_from_images(list))
  }
}

impl From<VideoListRef> for HiggsfieldMediaSources {
  fn from(list: VideoListRef) -> Self {
    Self(MediaSourceRef::list_from_videos(list))
  }
}

impl From<AudioListRef> for HiggsfieldMediaSources {
  fn from(list: AudioListRef) -> Self {
    Self(MediaSourceRef::list_from_audios(list))
  }
}

impl HiggsfieldMediaSources {
  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}

/// Everything an upload needs besides the source itself.
#[derive(Clone, Copy)]
pub struct HiggsfieldUploadContext<'a> {
  pub session: &'a HiggsfieldSession,
  /// Media file tokens resolve to ArtCraft CDN URLs through this.
  pub maybe_map: Option<&'a HashMap<MediaFileToken, String>>,
  /// The account's prior uploads; `None` uploads unconditionally.
  pub maybe_cache: Option<&'a dyn AssetUploadCache>,
  /// Told whether each source was reused or uploaded.
  pub maybe_observer: Option<&'a dyn AssetUploadObserver>,
}

/// Upload every source in `list`, in order. `None` / empty lists upload
/// nothing.
pub async fn upload_media_list(
  context: HiggsfieldUploadContext<'_>,
  list: Option<HiggsfieldMediaSources>,
  kind: HiggsfieldMediaKind,
  ip_check: bool,
) -> Result<Vec<MediaInput>, ArtcraftRouterError> {
  let sources = match list {
    None => return Ok(Vec::new()),
    Some(sources) => sources.0,
  };
  let mut uploaded = Vec::with_capacity(sources.len());
  for source in sources {
    uploaded.push(upload_source_to_higgsfield(context, source, kind, ip_check).await?);
  }
  Ok(uploaded)
}

/// Upload a single image reference (a keyframe), if present.
pub async fn upload_image_ref(
  context: HiggsfieldUploadContext<'_>,
  image_ref: Option<ImageRef>,
  ip_check: bool,
) -> Result<Option<MediaInput>, ArtcraftRouterError> {
  match image_ref {
    None => Ok(None),
    Some(image_ref) => {
      Ok(Some(upload_source_to_higgsfield(context, image_ref.into(), HiggsfieldMediaKind::Image, ip_check).await?))
    }
  }
}

/// Turn one source into bytes and get a Higgsfield reference for it: the
/// account's previous upload of the same bytes when the cache knows one and
/// Higgsfield confirms it, otherwise a fresh upload (recorded for next
/// time). `ip_check` asks Higgsfield to run (and waits for) its
/// intellectual-property check, which the Seedance video models require on
/// images and clips; a cached upload that never had it is uploaded again.
///
/// The first-party-domain guard applies only to caller-supplied `Url`
/// sources: a token that resolves to our own CDN is a deliberate library
/// pick, and local paths / bytes never had a URL at all.
pub async fn upload_source_to_higgsfield(
  context: HiggsfieldUploadContext<'_>,
  source: MediaSourceRef,
  kind: HiggsfieldMediaKind,
  ip_check: bool,
) -> Result<MediaInput, ArtcraftRouterError> {
  if let MediaSourceRef::Url(url) = &source {
    check_upload_source_url(url)
        .map_err(|err| ArtcraftRouterError::from(ProviderError::Higgsfield(err.into())))?;
  }
  let resolved = resolve_media_source_bytes(source, context.maybe_map).await?;

  let file_hash = hash_bytes_blake3(&resolved.bytes);
  if let Some(cache) = context.maybe_cache {
    if let Some(reusable) = reusable_upload(context.session, cache, &file_hash, kind, ip_check).await {
      info!("Reusing Higgsfield {:?} reference {} for {} (already uploaded by this account)", kind, reusable.id, resolved.description);
      cache.note_reused(&file_hash).await;
      if let Some(observer) = context.maybe_observer {
        observer.on_reused(kind.label(), &resolved.description);
      }
      return Ok(reusable);
    }
  }

  let byte_count = resolved.bytes.len() as u64;
  let file = reference_media_file(resolved, kind, ip_check);
  info!(
    "Uploading {:?} reference to Higgsfield ({} bytes, {}, ip_check={})",
    kind, file.bytes.len(), file.mime_type, ip_check,
  );
  if let Some(observer) = context.maybe_observer {
    observer.on_uploading(kind.label(), byte_count, ip_check);
  }
  let uploaded = session_upload(context.session, file).await?;

  if let Some(cache) = context.maybe_cache {
    cache.record_upload(&file_hash, byte_count, &CachedAssetUpload::uploaded(
      uploaded.id.as_str(),
      Some(uploaded.url.clone()),
    )).await;
  }
  Ok(uploaded)
}

async fn session_upload(session: &HiggsfieldSession, file: ReferenceMediaFile) -> Result<MediaInput, ArtcraftRouterError> {
  session.upload_reference_media(file).await
      .map_err(|err| ArtcraftRouterError::from(ProviderError::Higgsfield(err)))
}

/// The cached upload for `file_hash`, if Higgsfield still honours it. Any
/// problem — no cached URL, a failed status read, a missing or flagged
/// upload, or a required IP check that never ran — forgets the entry and
/// answers `None` so the caller uploads afresh.
async fn reusable_upload(
  session: &HiggsfieldSession,
  cache: &dyn AssetUploadCache,
  file_hash: &str,
  kind: HiggsfieldMediaKind,
  ip_check: bool,
) -> Option<MediaInput> {
  let cached = cache.find_upload(file_hash).await?;
  let media_id = MediaId::new(cached.service_id.clone());

  if cached.origin == AssetUploadOrigin::GenerationResult {
    return reusable_generation_result(session, cache, file_hash, kind, media_id).await;
  }

  let Some(url) = cached.maybe_service_url.clone() else {
    warn!("Cached Higgsfield upload {} has no URL; uploading again", media_id);
    cache.forget_upload(file_hash).await;
    return None;
  };

  if let Some(family) = kind.status_family() {
    match session.get_media_status(family, &media_id).await {
      Ok(status) if status.is_ip_detected() => {
        warn!("Cached Higgsfield upload {} is flagged as protected content; uploading again", media_id);
        cache.forget_upload(file_hash).await;
        return None;
      }
      Ok(status) if ip_check && !status.is_ip_check_finished() => {
        info!("Cached Higgsfield upload {} never had its IP check; uploading again with one", media_id);
        cache.forget_upload(file_hash).await;
        return None;
      }
      Ok(_) => {}
      Err(err) => {
        warn!("Could not confirm cached Higgsfield upload {} ({}); uploading again", media_id, err);
        cache.forget_upload(file_hash).await;
        return None;
      }
    }
  }

  Some(kind.media_input(media_id, url))
}

/// A file Higgsfield generated itself (we downloaded the result) is used as a
/// "previous generation" reference by job id — no upload, and no IP check,
/// since it never left Higgsfield. The job is confirmed to still exist with
/// a result, and the fresh result URL is used (CDN URLs are time-limited).
async fn reusable_generation_result(
  session: &HiggsfieldSession,
  cache: &dyn AssetUploadCache,
  file_hash: &str,
  kind: HiggsfieldMediaKind,
  job_id: MediaId,
) -> Option<MediaInput> {
  let job = match session.job_status(&JobId::new(job_id.as_str())).await {
    Ok(job) => job,
    Err(err) => {
      warn!("Could not confirm Higgsfield generation {} still exists ({}); uploading instead", job_id, err);
      cache.forget_upload(file_hash).await;
      return None;
    }
  };
  let Some(url) = job.result_url() else {
    warn!("Higgsfield generation {} no longer has a result; uploading instead", job_id);
    cache.forget_upload(file_hash).await;
    return None;
  };
  let Some(input) = kind.job_input(job_id.clone(), url.to_string()) else {
    info!("Higgsfield generation {} can't stand in for a {:?} reference; uploading instead", job_id, kind);
    return None;
  };
  Some(input)
}

/// Describe resolved bytes for upload: the MIME type is sniffed from the
/// bytes, then guessed from the name hint (URL, path, or file name), then
/// defaulted by `kind`. The resolved source URL (caller-supplied URLs only)
/// engages the client's first-party-domain guard.
pub fn reference_media_file(resolved: ResolvedMediaBytes, kind: HiggsfieldMediaKind, ip_check: bool) -> ReferenceMediaFile {
  let ResolvedMediaBytes { bytes, maybe_name_hint, maybe_source_url, description } = resolved;
  let mime_type = media_mime_type_for(maybe_name_hint.as_deref(), &bytes, kind);
  info!("Higgsfield reference resolved from {} as {}", description, mime_type);
  let file_name = format!("reference.{}", mime_type.file_extension());
  let mut file = ReferenceMediaFile::new(file_name, mime_type, bytes);
  if let Some(source_url) = maybe_source_url {
    file = file.with_source_url(source_url);
  }
  if ip_check { file.with_ip_check() } else { file }
}

/// The best MIME type for a reference: magic bytes first (the name may have
/// no extension, or lie), then the name hint's extension, then the kind's
/// default. A type from the wrong family for `kind` is ignored.
pub fn media_mime_type_for(maybe_name_hint: Option<&str>, bytes: &[u8], kind: HiggsfieldMediaKind) -> MediaMimeType {
  sniff_mime_type(bytes)
      .or_else(|| maybe_name_hint.and_then(mime_type_from_name_hint))
      .filter(|mime_type| kind.accepts(mime_type))
      .unwrap_or_else(|| kind.fallback_mime_type())
}

/// Extension-based MIME guess for a URL, filesystem path, or bare file name.
fn mime_type_from_name_hint(name_hint: &str) -> Option<MediaMimeType> {
  let extension = extension_from_name_hint(name_hint)?;
  MediaMimeType::from_file_name(&format!("file.{extension}"))
}

/// Recognise the common image / video / audio containers by their magic
/// bytes. Returns `None` for anything else.
pub fn sniff_mime_type(bytes: &[u8]) -> Option<MediaMimeType> {
  if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
    return Some(MediaMimeType::ImagePng);
  }
  if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
    return Some(MediaMimeType::ImageJpeg);
  }
  if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
    return Some(MediaMimeType::ImageGif);
  }
  if bytes.len() >= 12 && bytes.starts_with(b"RIFF") {
    return match &bytes[8..12] {
      b"WEBP" => Some(MediaMimeType::ImageWebp),
      b"WAVE" => Some(MediaMimeType::AudioWav),
      _ => None,
    };
  }
  if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
    // ISO base media: MP4 / MOV / M4A / HEIC share the box; brand tells.
    return match &bytes[8..12] {
      b"qt  " => Some(MediaMimeType::VideoQuicktime),
      b"M4A " => Some(MediaMimeType::AudioMp4),
      b"heic" | b"heix" | b"mif1" => Some(MediaMimeType::ImageHeic),
      b"avif" => Some(MediaMimeType::ImageAvif),
      _ => Some(MediaMimeType::VideoMp4),
    };
  }
  if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
    // Matroska / WebM. Assume the web flavour; it's what browsers produce.
    return Some(MediaMimeType::VideoWebm);
  }
  if bytes.starts_with(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0) {
    return Some(MediaMimeType::AudioMpeg);
  }
  if bytes.starts_with(b"OggS") {
    return Some(MediaMimeType::AudioOgg);
  }
  None
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use crate::api::media_bytes::MediaBytes;
  use crate::errors::client_error::ClientError;

  use super::*;

  const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
  const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
  const MP4: &[u8] = b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2";
  const MOV: &[u8] = b"\0\0\0\x14ftypqt  \0\0\0\0qt  ";
  const WAV: &[u8] = b"RIFF\x24\0\0\0WAVEfmt ";
  const WEBP: &[u8] = b"RIFF\x24\0\0\0WEBPVP8 ";
  const MP3_ID3: &[u8] = b"ID3\x04\0\0\0\0\0\0";

  mod sniffing {
    use super::*;

    #[test]
    fn recognises_common_containers() {
      assert_eq!(sniff_mime_type(PNG), Some(MediaMimeType::ImagePng));
      assert_eq!(sniff_mime_type(JPEG), Some(MediaMimeType::ImageJpeg));
      assert_eq!(sniff_mime_type(WEBP), Some(MediaMimeType::ImageWebp));
      assert_eq!(sniff_mime_type(MP4), Some(MediaMimeType::VideoMp4));
      assert_eq!(sniff_mime_type(MOV), Some(MediaMimeType::VideoQuicktime));
      assert_eq!(sniff_mime_type(WAV), Some(MediaMimeType::AudioWav));
      assert_eq!(sniff_mime_type(MP3_ID3), Some(MediaMimeType::AudioMpeg));
      assert_eq!(sniff_mime_type(&[0x1A, 0x45, 0xDF, 0xA3, 0x01]), Some(MediaMimeType::VideoWebm));
    }

    #[test]
    fn unknown_bytes_are_none() {
      assert_eq!(sniff_mime_type(b"hello world"), None);
      assert_eq!(sniff_mime_type(b""), None);
      assert_eq!(sniff_mime_type(b"RIFF\0\0\0\0AVI "), None);
    }
  }

  mod mime_resolution {
    use super::*;

    #[test]
    fn bytes_win_over_extension() {
      // A PNG served from a ".jpg" URL is still a PNG.
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/a.jpg"), PNG, HiggsfieldMediaKind::Image), MediaMimeType::ImagePng);
    }

    #[test]
    fn extension_is_used_when_bytes_are_unrecognised() {
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/a.webp?x=1"), b"????", HiggsfieldMediaKind::Image), MediaMimeType::ImageWebp);
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/clip.mov"), b"????", HiggsfieldMediaKind::Video), MediaMimeType::VideoQuicktime);
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/song.mp3"), b"????", HiggsfieldMediaKind::Audio), MediaMimeType::AudioMpeg);
    }

    #[test]
    fn local_paths_and_bare_names_give_extensions() {
      assert_eq!(media_mime_type_for(Some("/Users/me/Pictures/photo.webp"), b"????", HiggsfieldMediaKind::Image), MediaMimeType::ImageWebp);
      assert_eq!(media_mime_type_for(Some("clip.mov"), b"????", HiggsfieldMediaKind::Video), MediaMimeType::VideoQuicktime);
    }

    #[test]
    fn falls_back_by_kind() {
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/media/abc"), b"????", HiggsfieldMediaKind::Image), MediaMimeType::ImageJpeg);
      assert_eq!(media_mime_type_for(None, b"????", HiggsfieldMediaKind::Video), MediaMimeType::VideoMp4);
      assert_eq!(media_mime_type_for(None, b"????", HiggsfieldMediaKind::Audio), MediaMimeType::AudioMpeg);
    }

    #[test]
    fn wrong_family_for_kind_is_ignored() {
      // An image handed in as a video reference: don't claim it's a PNG video.
      assert_eq!(media_mime_type_for(Some("https://cdn.example.com/a.png"), PNG, HiggsfieldMediaKind::Video), MediaMimeType::VideoMp4);
    }

    #[test]
    fn reference_file_carries_extension_ip_check_and_source_url() {
      let resolved = ResolvedMediaBytes {
        bytes: MP4.to_vec(),
        maybe_name_hint: Some("https://cdn.example.com/x".into()),
        maybe_source_url: Some("https://cdn.example.com/x".into()),
        description: "url https://cdn.example.com/x".into(),
      };
      let file = reference_media_file(resolved, HiggsfieldMediaKind::Video, true);
      assert_eq!(file.file_name, "reference.mp4");
      assert_eq!(file.mime_type, MediaMimeType::VideoMp4);
      assert!(file.force_ip_check);
      assert_eq!(file.maybe_source_url.as_deref(), Some("https://cdn.example.com/x"));

      let resolved = ResolvedMediaBytes {
        bytes: PNG.to_vec(),
        maybe_name_hint: Some("/local/x.png".into()),
        maybe_source_url: None,
        description: "local file /local/x.png".into(),
      };
      let file = reference_media_file(resolved, HiggsfieldMediaKind::Image, false);
      assert_eq!(file.file_name, "reference.png");
      assert!(!file.force_ip_check);
      assert!(file.maybe_source_url.is_none());
    }
  }

  mod upload_reuse {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use higgsfield_client::client::clerk_host::ClerkHost;
    use higgsfield_client::client::higgsfield_host::HiggsfieldHost;
    use higgsfield_client::types::media_input::MediaInputKind;

    use super::*;

    /// A cache with one canned entry that records what the router did.
    struct CannedCache {
      entry: Option<CachedAssetUpload>,
      forgotten: Mutex<Vec<String>>,
      reused: Mutex<Vec<String>>,
    }

    impl CannedCache {
      fn with(entry: CachedAssetUpload) -> Self {
        Self { entry: Some(entry), forgotten: Mutex::new(Vec::new()), reused: Mutex::new(Vec::new()) }
      }
    }

    #[async_trait]
    impl AssetUploadCache for CannedCache {
      async fn find_upload(&self, _file_hash_blake3: &str) -> Option<CachedAssetUpload> {
        self.entry.clone()
      }
      async fn record_upload(&self, _file_hash_blake3: &str, _file_size_bytes: u64, _upload: &CachedAssetUpload) {}
      async fn note_reused(&self, file_hash_blake3: &str) {
        self.reused.lock().unwrap().push(file_hash_blake3.to_string());
      }
      async fn forget_upload(&self, file_hash_blake3: &str) {
        self.forgotten.lock().unwrap().push(file_hash_blake3.to_string());
      }
    }

    /// A session whose every request fails (nothing listens on port 9).
    fn unreachable_session() -> HiggsfieldSession {
      HiggsfieldSession::from_cookie_header("__client=x")
          .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()))
    }

    #[tokio::test]
    async fn audio_has_no_status_endpoint_so_the_cached_id_is_trusted() {
      let cache = CannedCache::with(CachedAssetUpload::uploaded("media-audio", Some("https://cdn/a.mp3".into())));
      let reused = reusable_upload(&unreachable_session(), &cache, "hash", HiggsfieldMediaKind::Audio, false).await.unwrap();
      assert_eq!(reused.id.as_str(), "media-audio");
      assert_eq!(reused.kind, MediaInputKind::AudioInput);
      assert_eq!(reused.url, "https://cdn/a.mp3");
      assert!(cache.forgotten.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_entry_without_a_url_is_forgotten() {
      let cache = CannedCache::with(CachedAssetUpload::uploaded("media-1", None));
      assert!(reusable_upload(&unreachable_session(), &cache, "hash", HiggsfieldMediaKind::Audio, false).await.is_none());
      assert_eq!(*cache.forgotten.lock().unwrap(), vec!["hash".to_string()]);
    }

    #[tokio::test]
    async fn a_generation_result_that_cannot_be_confirmed_means_a_fresh_upload() {
      let cache = CannedCache::with(CachedAssetUpload::generation_result("job-1", "https://cdn/result.png"));
      let result = reusable_upload(&unreachable_session(), &cache, "hash", HiggsfieldMediaKind::Image, true).await;
      assert!(result.is_none());
      assert!(cache.forgotten.lock().unwrap().contains(&"hash".to_string()));
    }

    #[test]
    fn generation_results_become_job_references_by_kind() {
      let image = HiggsfieldMediaKind::Image.job_input(MediaId::new("job-1"), "https://cdn/r.png".into()).unwrap();
      assert_eq!(image.kind, MediaInputKind::ImageJob);
      assert_eq!(image.id.as_str(), "job-1");
      let video = HiggsfieldMediaKind::Video.job_input(MediaId::new("job-2"), "https://cdn/r.mp4".into()).unwrap();
      assert_eq!(video.kind, MediaInputKind::VideoJob);
      assert!(HiggsfieldMediaKind::Audio.job_input(MediaId::new("job-3"), "https://cdn/r.mp3".into()).is_none());
    }

    #[tokio::test]
    async fn a_status_check_that_fails_means_a_fresh_upload() {
      let cache = CannedCache::with(CachedAssetUpload::uploaded("media-1", Some("https://cdn/x.png".into())));
      assert!(reusable_upload(&unreachable_session(), &cache, "hash", HiggsfieldMediaKind::Image, true).await.is_none());
      assert_eq!(*cache.forgotten.lock().unwrap(), vec!["hash".to_string()]);
      assert!(cache.reused.lock().unwrap().is_empty());
    }

    #[test]
    fn media_inputs_take_their_kind_from_the_reference_kind() {
      let id = MediaId::new("abc");
      assert_eq!(HiggsfieldMediaKind::Image.media_input(id.clone(), "u".into()).kind, MediaInputKind::MediaInput);
      assert_eq!(HiggsfieldMediaKind::Video.media_input(id.clone(), "u".into()).kind, MediaInputKind::VideoInput);
      assert_eq!(HiggsfieldMediaKind::Audio.media_input(id, "u".into()).kind, MediaInputKind::AudioInput);
    }
  }

  mod token_resolution {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn tokens_resolve_through_the_map() {
      let token = MediaFileToken::new_from_str("m_abc");
      let mut map = HashMap::new();
      map.insert(token.clone(), "https://cdn.example.com/abc.png".to_string());
      assert_eq!(resolve_media_token(Some(&map), &token).unwrap(), "https://cdn.example.com/abc.png");
      assert!(matches!(
        resolve_media_token(None, &token),
        Err(ArtcraftRouterError::Client(ClientError::MediaFileToUrlMapNotProvided)),
      ));
      assert!(matches!(
        resolve_media_token(Some(&map), &MediaFileToken::new_from_str("m_missing")),
        Err(ArtcraftRouterError::Client(ClientError::MediaFileTokenNotFoundInMap { .. })),
      ));
    }

    #[test]
    fn source_lists_convert_from_every_ref_type() {
      let images: HiggsfieldMediaSources = ImageListRef::Urls(vec!["a".into(), "b".into()]).into();
      assert_eq!(images.len(), 2);
      let videos: HiggsfieldMediaSources = VideoListRef::MediaFileTokens(vec![MediaFileToken::new_from_str("m_1")]).into();
      assert_eq!(videos.len(), 1);
      let audio: HiggsfieldMediaSources = AudioListRef::Urls(vec![]).into();
      assert!(audio.is_empty());
      let mixed: HiggsfieldMediaSources = ImageListRef::Sources(vec![
        ImageRef::MediaFileToken(MediaFileToken::new_from_str("m_2")),
        ImageRef::LocalPath(PathBuf::from("/tmp/a.png")),
        ImageRef::Bytes(MediaBytes::new(vec![1, 2, 3])),
      ]).into();
      assert_eq!(mixed.len(), 3);
      assert!(matches!(mixed.0[1], MediaSourceRef::LocalPath(_)));
      assert!(matches!(mixed.0[2], MediaSourceRef::Bytes(_)));
    }
  }
}
