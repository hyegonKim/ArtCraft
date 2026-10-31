//! Upload reference files through a [`HiggsfieldSession`]: presign, `PUT`
//! the bytes, confirm — and hand back the [`MediaInput`] descriptor a
//! generation request wants.

use crate::endpoints::media::confirm_audio_upload::{confirm_audio_upload, ConfirmAudioUploadArgs, ConfirmAudioUploadRequest};
use crate::endpoints::media::confirm_media_upload::{confirm_media_upload, ConfirmMediaUploadArgs, ConfirmMediaUploadRequest, ConfirmMediaUploadResponse};
use crate::endpoints::media::confirm_video_upload::{confirm_video_upload, ConfirmVideoUploadArgs, ConfirmVideoUploadRequest};
use crate::endpoints::media::create_audio_upload::{create_audio_upload, AudioUploadExtension, CreateAudioUploadArgs, CreateAudioUploadRequest};
use crate::endpoints::media::create_media_batch::{create_media_batch, CreateMediaBatchArgs, CreateMediaBatchRequest};
use crate::endpoints::media::create_reference_media::{create_reference_media, CreateReferenceMediaArgs, CreateReferenceMediaRequest};
use crate::endpoints::media::create_video_upload::{create_video_upload, CreateVideoUploadArgs, CreateVideoUploadRequest};
use crate::endpoints::media::get_media_status::{get_media_status, GetMediaStatusArgs, GetMediaStatusRequest, GetMediaStatusResponse, MediaStatusFamily};
use crate::endpoints::media::upload_media_bytes::{upload_media_bytes, UploadMediaBytesArgs, UploadMediaBytesRequest};
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::session::upload_source_guard::check_upload_source_url;
use crate::types::ids::MediaId;
use crate::types::media_input::MediaInput;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;
use log::info;
use std::time::{Duration, Instant};

/// How long [`HiggsfieldSession::wait_for_media_ip_check`] polls by default.
const DEFAULT_IP_CHECK_TIMEOUT: Duration = Duration::from_secs(90);

const IP_CHECK_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A file to upload as reference media.
#[derive(Clone, Debug)]
pub struct ReferenceMediaFile {
  /// Shown in the web app's media library; its extension is also the
  /// fallback for guessing `mime_type`.
  pub file_name: String,

  pub mime_type: MediaMimeType,

  pub bytes: Vec<u8>,

  /// Ask the server to run its intellectual-property check on confirm,
  /// and wait for it to finish before returning. Off by default (like the
  /// web app's frame pickers). Required for models that refuse unchecked
  /// media — Seedance 2.x answers `400 "IP check not finished for input
  /// media"` otherwise. Images and video only; audio has no IP check.
  pub force_ip_check: bool,

  /// Where the bytes were downloaded from, when they came from a URL. Set
  /// it so uploads sourced from first-party domains are refused — see
  /// `upload_source_guard`.
  pub maybe_source_url: Option<String>,
}

impl ReferenceMediaFile {
  pub fn new(file_name: impl Into<String>, mime_type: MediaMimeType, bytes: Vec<u8>) -> Self {
    Self { file_name: file_name.into(), mime_type, bytes, force_ip_check: false, maybe_source_url: None }
  }

  /// Run and wait for the IP check (see [`Self::force_ip_check`]).
  pub fn with_ip_check(mut self) -> Self {
    self.force_ip_check = true;
    self
  }

  /// Record where the bytes were downloaded from (see
  /// [`Self::maybe_source_url`]).
  pub fn with_source_url(mut self, source_url: impl Into<String>) -> Self {
    self.maybe_source_url = Some(source_url.into());
    self
  }

  /// Guess the type from the file name's extension.
  pub fn from_file_name(file_name: impl Into<String>, bytes: Vec<u8>) -> Result<Self, HiggsfieldClientError> {
    let file_name = file_name.into();
    let mime_type = MediaMimeType::from_file_name(&file_name)
        .ok_or_else(|| HiggsfieldClientError::InvalidRequest(format!("can't tell the media type of {file_name:?}; pass it explicitly")))?;
    Ok(Self { file_name, mime_type, bytes, force_ip_check: false, maybe_source_url: None })
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if let Some(source_url) = &self.maybe_source_url {
      check_upload_source_url(source_url)?;
    }
    if self.file_name.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("file_name is empty".to_string()));
    }
    if self.bytes.is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest(format!("{:?} has no bytes", self.file_name)));
    }
    if self.mime_type.is_audio() && AudioUploadExtension::for_mime_type(&self.mime_type).is_none() {
      return Err(HiggsfieldClientError::InvalidRequest(format!(
        "{:?}: audio uploads support only wav, mp3 or webm, not {}", self.file_name, self.mime_type,
      )));
    }
    Ok(())
  }

  fn family(&self) -> MediaFamily {
    if self.mime_type.is_video() {
      MediaFamily::Video
    } else if self.mime_type.is_audio() {
      MediaFamily::Audio
    } else {
      MediaFamily::Image
    }
  }
}

/// Which presign / confirm pair a file goes through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MediaFamily {
  Image,
  Video,
  Audio,
}

impl HiggsfieldSession {
  /// Upload one file — image, video or audio — and get back the descriptor
  /// to reference it with. Picks the presign / confirm pair for the file's
  /// type (`/fnf/reference-media`, `/fnf/video`, `/fnf/audio`).
  ///
  /// ```ignore
  /// let reference = session.upload_reference_media(ReferenceMediaFile::from_file_name("cat.png", bytes)?).await?;
  /// let request = NanoBanana2Request::text_to_image("the same cat, but on the moon", NanoBananaAspectRatio::Auto, NanoBanana2Resolution::OneK)
  ///     .with_reference_images(vec![reference]);
  /// ```
  pub async fn upload_reference_media(&self, file: ReferenceMediaFile) -> Result<MediaInput, HiggsfieldError> {
    file.validate()?;

    let slot = match file.family() {
      MediaFamily::Image => self.with_auth(|auth| {
        let request = CreateReferenceMediaRequest::new(file.mime_type.clone());
        async move { create_reference_media(CreateReferenceMediaArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
      MediaFamily::Video => self.with_auth(|auth| {
        let request = CreateVideoUploadRequest::new(file.mime_type.clone());
        async move { create_video_upload(CreateVideoUploadArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
      MediaFamily::Audio => self.with_auth(|auth| {
        let extension = AudioUploadExtension::for_mime_type(&file.mime_type).expect("validated above");
        let request = CreateAudioUploadRequest::new(extension, file.file_name.clone());
        async move { create_audio_upload(CreateAudioUploadArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
    };

    self.upload_and_confirm(&slot, file).await
  }

  /// Upload several image files in one presign round trip, in order. Uses
  /// the video generator's presign (`/fnf/media/batch`), which only takes
  /// image types; upload video / audio one at a time with
  /// [`Self::upload_reference_media`].
  pub async fn upload_reference_media_batch(&self, files: Vec<ReferenceMediaFile>) -> Result<Vec<MediaInput>, HiggsfieldError> {
    if files.is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("no files to upload".to_string()).into());
    }
    for file in &files {
      file.validate()?;
    }

    let mime_types: Vec<MediaMimeType> = files.iter().map(|file| file.mime_type.clone()).collect();

    let slots = self.with_auth(|auth| {
      let request = CreateMediaBatchRequest::new(mime_types.clone());
      async move { create_media_batch(CreateMediaBatchArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await?;

    if slots.len() != files.len() {
      return Err(HiggsfieldClientError::InvalidRequest(format!(
        "the server allocated {} upload slots for {} files", slots.len(), files.len(),
      )).into());
    }

    let mut uploaded = Vec::with_capacity(files.len());
    for (slot, file) in slots.iter().zip(files) {
      uploaded.push(self.upload_and_confirm(slot, file).await?);
    }
    Ok(uploaded)
  }

  // ── Private ──

  async fn upload_and_confirm(&self, slot: &PresignedMediaUpload, file: ReferenceMediaFile) -> Result<MediaInput, HiggsfieldError> {
    let family = file.family();
    let ReferenceMediaFile { file_name, mime_type, bytes, force_ip_check, .. } = file;
    let byte_count = bytes.len();

    upload_media_bytes(UploadMediaBytesArgs {
      request: UploadMediaBytesRequest::for_slot(slot, bytes),
      maybe_user_agent: self.maybe_user_agent(),
    }).await?;

    let confirmation: ConfirmMediaUploadResponse = match family {
      MediaFamily::Image => self.with_auth(|auth| {
        let mut request = ConfirmMediaUploadRequest::new(slot.id.clone(), file_name.clone());
        request.force_ip_check = force_ip_check;
        async move { confirm_media_upload(ConfirmMediaUploadArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
      MediaFamily::Video => self.with_auth(|auth| {
        let mut request = ConfirmVideoUploadRequest::new(slot.id.clone());
        request.maybe_force_ip_check = force_ip_check.then_some(true);
        async move { confirm_video_upload(ConfirmVideoUploadArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
      MediaFamily::Audio => self.with_auth(|auth| {
        let request = ConfirmAudioUploadRequest::new(slot.id.clone());
        async move { confirm_audio_upload(ConfirmAudioUploadArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?,
    };

    info!(
      "Higgsfield reference media {} uploaded ({} bytes, {}, status {})",
      slot.id, byte_count, mime_type, confirmation.status,
    );

    if confirmation.is_ip_detected() {
      return Err(HiggsfieldClientError::MediaProtectedContent { media_id: slot.id.clone() }.into());
    }

    if force_ip_check && !confirmation.ip_check_finished.unwrap_or(false) {
      let status_family = match family {
        MediaFamily::Image => Some(MediaStatusFamily::Image),
        MediaFamily::Video => Some(MediaStatusFamily::Video),
        MediaFamily::Audio => None,
      };
      if let Some(status_family) = status_family {
        self.wait_for_media_ip_check(status_family, &slot.id, DEFAULT_IP_CHECK_TIMEOUT).await?;
      }
    }

    Ok(slot.to_media_input())
  }
}

impl HiggsfieldSession {
  /// One status read of an uploaded image or clip (`GET /fnf/media/{id}` /
  /// `/fnf/video/{id}`). Used to confirm a previously uploaded reference
  /// still exists (and whether its IP check ran) before reusing its id.
  pub async fn get_media_status(&self, family: MediaStatusFamily, media_id: &MediaId) -> Result<GetMediaStatusResponse, HiggsfieldError> {
    self.with_auth(|auth| {
      let request = GetMediaStatusRequest::new(family, media_id.clone());
      async move { get_media_status(GetMediaStatusArgs { request, auth: &auth, host: self.api_host() }).await }
    }).await
  }

  /// Poll the upload's status (`GET /fnf/media/{id}` for images,
  /// `/fnf/video/{id}` for clips) until its IP check has finished. Only
  /// completes if the check was requested (`force_ip_check` on confirm);
  /// otherwise it times out with
  /// [`HiggsfieldClientError::MediaIpCheckTimedOut`].
  pub async fn wait_for_media_ip_check(&self, family: MediaStatusFamily, media_id: &MediaId, timeout: Duration) -> Result<GetMediaStatusResponse, HiggsfieldError> {
    let started = Instant::now();
    loop {
      let status = self.with_auth(|auth| {
        let request = GetMediaStatusRequest::new(family, media_id.clone());
        async move { get_media_status(GetMediaStatusArgs { request, auth: &auth, host: self.api_host() }).await }
      }).await?;

      if status.is_ip_detected() {
        return Err(HiggsfieldClientError::MediaProtectedContent { media_id: media_id.clone() }.into());
      }
      if status.is_ip_check_finished() {
        info!("Higgsfield media {} IP check finished after {}s (ip_detected={:?})", media_id, started.elapsed().as_secs(), status.ip_detected);
        return Ok(status);
      }
      if started.elapsed() >= timeout {
        return Err(HiggsfieldClientError::MediaIpCheckTimedOut { media_id: media_id.clone(), waited: started.elapsed() }.into());
      }
      tokio::time::sleep(IP_CHECK_POLL_INTERVAL).await;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn file_from_name_guesses_the_type() {
    let file = ReferenceMediaFile::from_file_name("frame.JPG", vec![1]).unwrap();
    assert_eq!(file.mime_type, MediaMimeType::ImageJpeg);
    assert!(matches!(ReferenceMediaFile::from_file_name("notes.txt", vec![1]), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    assert!(matches!(ReferenceMediaFile::new("a.aac", MediaMimeType::AudioAac, vec![1]).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert_eq!(ReferenceMediaFile::new("a.mov", MediaMimeType::VideoQuicktime, vec![1]).family(), MediaFamily::Video);
    assert_eq!(ReferenceMediaFile::new("a.mp3", MediaMimeType::AudioMpeg, vec![1]).family(), MediaFamily::Audio);
    assert!(matches!(ReferenceMediaFile::new("a.png", MediaMimeType::ImagePng, vec![]).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(matches!(ReferenceMediaFile::new(" ", MediaMimeType::ImagePng, vec![1]).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(ReferenceMediaFile::new("a.png", MediaMimeType::ImagePng, vec![1]).validate().is_ok());
  }

  #[test]
  fn first_party_source_urls_are_refused() {
    let file = ReferenceMediaFile::new("a.png", MediaMimeType::ImagePng, vec![1])
        .with_source_url("https://cdn-2.fakeyou.com/media/a/image_a.png");
    assert!(matches!(file.validate(), Err(HiggsfieldClientError::UploadSourceDomainBlocked { .. })));
    let file = ReferenceMediaFile::new("a.png", MediaMimeType::ImagePng, vec![1])
        .with_source_url("https://cdn.example.com/a.png");
    assert!(file.validate().is_ok());
  }

  #[tokio::test]
  async fn empty_inputs_fail_before_network() {
    use crate::client::clerk_host::ClerkHost;
    use crate::client::higgsfield_host::HiggsfieldHost;
    let session = HiggsfieldSession::from_cookie_header("__client=x")
        .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()));

    let err = session.upload_reference_media(ReferenceMediaFile::new("a.png", MediaMimeType::ImagePng, vec![])).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));

    let err = session.upload_reference_media_batch(vec![]).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }
}
