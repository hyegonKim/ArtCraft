//! A provider-agnostic view of one media reference, and the machinery to
//! turn it into bytes. The per-kind ref enums (`ImageRef`, `VideoRef`,
//! `AudioRef`, `MeshRef`) all collapse into [`MediaSourceRef`] for providers
//! that re-upload media (Higgsfield, Kinovi).
//!
//! Only ArtCraft media tokens touch the ArtCraft cloud (they resolve through
//! the token→URL map); local files and bytes never leave the machine until
//! they're uploaded to the target provider.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use url_utils::extension::extract_extension_from_url::{extract_extension_from_url_str, ExtractExtensions};

use crate::api::audio_list_ref::AudioListRef;
use crate::api::audio_ref::AudioRef;
use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::media_bytes::MediaBytes;
use crate::api::mesh_ref::MeshRef;
use crate::api::video_list_ref::VideoListRef;
use crate::api::video_ref::VideoRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use crate::utils::download_file::download_file;

/// One media reference, stripped of its image/video/audio kind.
#[derive(Clone, Debug)]
pub enum MediaSourceRef {
  MediaFileToken(MediaFileToken),
  Url(String),
  LocalPath(PathBuf),
  Bytes(MediaBytes),
}

/// A reference resolved to bytes, plus what's known about where it came from.
#[derive(Debug)]
pub struct ResolvedMediaBytes {
  pub bytes: Vec<u8>,

  /// URL, filesystem path, or bare file name — whatever can lend an
  /// extension for MIME/extension guessing.
  pub maybe_name_hint: Option<String>,

  /// Set only for caller-supplied `Url` sources; feeds provider-side source
  /// guards. Token-resolved CDN URLs deliberately don't set this: a token is
  /// a library pick, not a leaked local file.
  pub maybe_source_url: Option<String>,

  /// Human-readable origin for logs ("local file /a/b.png", ...).
  pub description: String,
}

impl MediaSourceRef {
  pub fn list_from_images(list: ImageListRef) -> Vec<MediaSourceRef> {
    list.into_refs().into_iter().map(Into::into).collect()
  }

  pub fn list_from_videos(list: VideoListRef) -> Vec<MediaSourceRef> {
    list.into_refs().into_iter().map(Into::into).collect()
  }

  pub fn list_from_audios(list: AudioListRef) -> Vec<MediaSourceRef> {
    list.into_refs().into_iter().map(Into::into).collect()
  }
}

impl From<ImageRef> for MediaSourceRef {
  fn from(image_ref: ImageRef) -> Self {
    match image_ref {
      ImageRef::MediaFileToken(token) => Self::MediaFileToken(token),
      ImageRef::Url(url) => Self::Url(url),
      ImageRef::LocalPath(path) => Self::LocalPath(path),
      ImageRef::Bytes(bytes) => Self::Bytes(bytes),
    }
  }
}

impl From<VideoRef> for MediaSourceRef {
  fn from(video_ref: VideoRef) -> Self {
    match video_ref {
      VideoRef::MediaFileToken(token) => Self::MediaFileToken(token),
      VideoRef::Url(url) => Self::Url(url),
      VideoRef::LocalPath(path) => Self::LocalPath(path),
      VideoRef::Bytes(bytes) => Self::Bytes(bytes),
    }
  }
}

impl From<AudioRef> for MediaSourceRef {
  fn from(audio_ref: AudioRef) -> Self {
    match audio_ref {
      AudioRef::MediaFileToken(token) => Self::MediaFileToken(token),
      AudioRef::Url(url) => Self::Url(url),
      AudioRef::LocalPath(path) => Self::LocalPath(path),
      AudioRef::Bytes(bytes) => Self::Bytes(bytes),
    }
  }
}

impl From<MeshRef> for MediaSourceRef {
  fn from(mesh_ref: MeshRef) -> Self {
    match mesh_ref {
      MeshRef::MediaFileToken(token) => Self::MediaFileToken(token),
      MeshRef::Url(url) => Self::Url(url),
      MeshRef::LocalPath(path) => Self::LocalPath(path),
      MeshRef::Bytes(bytes) => Self::Bytes(bytes),
    }
  }
}

/// Turn one source into bytes: tokens resolve through `maybe_map` and
/// download; URLs download; local paths read from disk; bytes pass through.
pub async fn resolve_media_source_bytes(
  source: MediaSourceRef,
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
) -> Result<ResolvedMediaBytes, ArtcraftRouterError> {
  match source {
    MediaSourceRef::MediaFileToken(token) => {
      let url = resolve_media_token(maybe_map, &token)?;
      let bytes = download_file(&url).await?;
      Ok(ResolvedMediaBytes {
        bytes,
        description: format!("media token {}", token.as_str()),
        maybe_name_hint: Some(url),
        maybe_source_url: None,
      })
    }
    MediaSourceRef::Url(url) => {
      let bytes = download_file(&url).await?;
      Ok(ResolvedMediaBytes {
        bytes,
        description: format!("url {url}"),
        maybe_name_hint: Some(url.clone()),
        maybe_source_url: Some(url),
      })
    }
    MediaSourceRef::LocalPath(path) => {
      let bytes = read_local_file(&path).await?;
      Ok(ResolvedMediaBytes {
        bytes,
        description: format!("local file {}", path.display()),
        maybe_name_hint: Some(path.to_string_lossy().into_owned()),
        maybe_source_url: None,
      })
    }
    MediaSourceRef::Bytes(media_bytes) => {
      let MediaBytes { bytes, maybe_file_name } = media_bytes;
      Ok(ResolvedMediaBytes {
        bytes,
        description: "raw bytes".to_string(),
        maybe_name_hint: maybe_file_name,
        maybe_source_url: None,
      })
    }
  }
}

/// Read a local reference file, distinguishing "not there" from "unreadable".
pub async fn read_local_file(path: &Path) -> Result<Vec<u8>, ArtcraftRouterError> {
  if !path.is_file() {
    return Err(ArtcraftRouterError::Client(ClientError::LocalFileNotFound { path: path.to_path_buf() }));
  }
  tokio::fs::read(path).await.map_err(|error| {
    ArtcraftRouterError::Client(ClientError::LocalFileRead { path: path.to_path_buf(), error })
  })
}

/// The extension a name hint (URL, filesystem path, or bare file name)
/// carries, without its period. `None` when there isn't one.
pub fn extension_from_name_hint(name_hint: &str) -> Option<String> {
  if let Some(extension) = extract_extension_from_url_str(name_hint, &ExtractExtensions::All) {
    return Some(extension.without_period().to_string());
  }
  Path::new(name_hint).extension()?.to_str().map(str::to_string)
}

pub fn resolve_media_token(
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
  token: &MediaFileToken,
) -> Result<String, ArtcraftRouterError> {
  let map = maybe_map.ok_or(ArtcraftRouterError::Client(ClientError::MediaFileToUrlMapNotProvided))?;
  map.get(token).cloned().ok_or_else(|| {
    ArtcraftRouterError::Client(ClientError::MediaFileTokenNotFoundInMap { token: token.clone() })
  })
}

pub fn resolve_media_tokens(
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
  tokens: &[MediaFileToken],
) -> Result<Vec<String>, ArtcraftRouterError> {
  tokens.iter().map(|token| resolve_media_token(maybe_map, token)).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extensions_come_from_urls_paths_and_names() {
    assert_eq!(extension_from_name_hint("https://cdn.example.com/a.webp?x=1").as_deref(), Some("webp"));
    assert_eq!(extension_from_name_hint("/Users/me/Pictures/photo.PNG").as_deref(), Some("PNG"));
    assert_eq!(extension_from_name_hint("clip.mov").as_deref(), Some("mov"));
    assert_eq!(extension_from_name_hint("no_extension"), None);
  }

  #[test]
  fn lists_collapse_to_sources() {
    let sources = MediaSourceRef::list_from_images(ImageListRef::Sources(vec![
      ImageRef::Url("https://a.example/a.png".into()),
      ImageRef::LocalPath(PathBuf::from("/tmp/b.png")),
      ImageRef::Bytes(MediaBytes::new(vec![1])),
    ]));
    assert_eq!(sources.len(), 3);
    assert!(matches!(sources[0], MediaSourceRef::Url(_)));
    assert!(matches!(sources[1], MediaSourceRef::LocalPath(_)));
    assert!(matches!(sources[2], MediaSourceRef::Bytes(_)));
  }

  #[tokio::test]
  async fn missing_local_file_is_a_clear_error() {
    let err = read_local_file(Path::new("/definitely/not/a/real/file.png")).await.unwrap_err();
    assert!(matches!(err, ArtcraftRouterError::Client(ClientError::LocalFileNotFound { .. })));
  }

  #[tokio::test]
  async fn bytes_pass_straight_through() {
    let resolved = resolve_media_source_bytes(
      MediaSourceRef::Bytes(MediaBytes::new(vec![9, 9]).with_file_name("a.png")),
      None,
    ).await.unwrap();
    assert_eq!(resolved.bytes, vec![9, 9]);
    assert_eq!(resolved.maybe_name_hint.as_deref(), Some("a.png"));
    assert!(resolved.maybe_source_url.is_none());
  }
}
