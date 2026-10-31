//! The Tauri bridge's three-way media source: reference media arrives from
//! the frontend as raw bytes, a local filesystem path, or an ArtCraft media
//! file token (a cloud-library pick). Local files and bytes never touch the
//! ArtCraft cloud unless the target provider itself requires it (ArtCraft
//! generations, and FAL — the one provider allowed to route through ArtCraft
//! uploads).

use std::fmt::{Debug, Formatter};
use std::path::PathBuf;

use router::api::audio_ref::AudioRef;
use router::api::image_ref::ImageRef;
use router::api::media_bytes::MediaBytes;
use router::api::video_ref::VideoRef;
use serde_derive::Deserialize;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::generate_error::{BadInputReason, GenerateError};

/// One media reference from the frontend.
///
/// Wire format (don't change without coordinating with the frontend):
/// `{"kind": "media_file_token", "token": "m_..."}`,
/// `{"kind": "local_path", "path": "/Users/..."}`, or
/// `{"kind": "bytes", "bytes": [...], "file_name": "photo.png"}`.
#[derive(Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TauriMediaSource {
  MediaFileToken { token: MediaFileToken },
  LocalPath { path: PathBuf },
  Bytes {
    bytes: Vec<u8>,
    #[serde(default)]
    file_name: Option<String>,
  },
}

impl TauriMediaSource {
  /// Reject unusable sources up front, before any provider work: a local
  /// path must point at an existing regular file, and bytes must be
  /// non-empty.
  pub fn validate(&self) -> Result<(), GenerateError> {
    match self {
      Self::MediaFileToken { .. } => Ok(()),
      Self::LocalPath { path } => {
        if path.is_file() {
          Ok(())
        } else {
          Err(GenerateError::BadInput(BadInputReason::LocalMediaFileNotFound { path: path.clone() }))
        }
      }
      Self::Bytes { bytes, .. } => {
        if bytes.is_empty() {
          Err(GenerateError::BadInput(BadInputReason::EmptyMediaBytes))
        } else {
          Ok(())
        }
      }
    }
  }

  pub fn maybe_media_file_token(&self) -> Option<&MediaFileToken> {
    match self {
      Self::MediaFileToken { token } => Some(token),
      Self::LocalPath { .. } | Self::Bytes { .. } => None,
    }
  }

  pub fn into_image_ref(self) -> ImageRef {
    match self {
      Self::MediaFileToken { token } => ImageRef::MediaFileToken(token),
      Self::LocalPath { path } => ImageRef::LocalPath(path),
      Self::Bytes { bytes, file_name } => ImageRef::Bytes(media_bytes(bytes, file_name)),
    }
  }

  pub fn into_video_ref(self) -> VideoRef {
    match self {
      Self::MediaFileToken { token } => VideoRef::MediaFileToken(token),
      Self::LocalPath { path } => VideoRef::LocalPath(path),
      Self::Bytes { bytes, file_name } => VideoRef::Bytes(media_bytes(bytes, file_name)),
    }
  }

  pub fn into_audio_ref(self) -> AudioRef {
    match self {
      Self::MediaFileToken { token } => AudioRef::MediaFileToken(token),
      Self::LocalPath { path } => AudioRef::LocalPath(path),
      Self::Bytes { bytes, file_name } => AudioRef::Bytes(media_bytes(bytes, file_name)),
    }
  }
}

// Manual impl: generate requests are logged at INFO; never dump the bytes.
impl Debug for TauriMediaSource {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MediaFileToken { token } => f.debug_struct("MediaFileToken").field("token", token).finish(),
      Self::LocalPath { path } => f.debug_struct("LocalPath").field("path", path).finish(),
      Self::Bytes { bytes, file_name } => f.debug_struct("Bytes")
          .field("bytes", &format_args!("<{} bytes>", bytes.len()))
          .field("file_name", file_name)
          .finish(),
    }
  }
}

/// A source field wins over its legacy token twin; legacy tokens are folded
/// into sources so handlers only ever see [`TauriMediaSource`].
pub fn merge_source_with_legacy_token(
  maybe_source: Option<TauriMediaSource>,
  maybe_legacy_token: Option<MediaFileToken>,
) -> Option<TauriMediaSource> {
  maybe_source.or_else(|| maybe_legacy_token.map(|token| TauriMediaSource::MediaFileToken { token }))
}

/// List twin of [`merge_source_with_legacy_token`]. An empty legacy list is
/// treated like an absent one.
pub fn merge_sources_with_legacy_tokens(
  maybe_sources: Option<Vec<TauriMediaSource>>,
  maybe_legacy_tokens: Option<Vec<MediaFileToken>>,
) -> Option<Vec<TauriMediaSource>> {
  maybe_sources.or_else(|| {
    let tokens = maybe_legacy_tokens?;
    if tokens.is_empty() {
      return None;
    }
    Some(tokens.into_iter().map(|token| TauriMediaSource::MediaFileToken { token }).collect())
  })
}

/// Validate every source a request carries; call once at command entry.
pub fn validate_sources<'a>(sources: impl IntoIterator<Item = &'a TauriMediaSource>) -> Result<(), GenerateError> {
  for source in sources {
    source.validate()?;
  }
  Ok(())
}

fn media_bytes(bytes: Vec<u8>, maybe_file_name: Option<String>) -> MediaBytes {
  match maybe_file_name {
    Some(file_name) => MediaBytes::new(bytes).with_file_name(file_name),
    None => MediaBytes::new(bytes),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_format_round_trip() {
    let source: TauriMediaSource = serde_json::from_str(r#"{"kind":"media_file_token","token":"m_abc"}"#).unwrap();
    assert!(matches!(&source, TauriMediaSource::MediaFileToken { token } if token.as_str() == "m_abc"));

    let source: TauriMediaSource = serde_json::from_str(r#"{"kind":"local_path","path":"/tmp/a.png"}"#).unwrap();
    assert!(matches!(&source, TauriMediaSource::LocalPath { path } if path == &PathBuf::from("/tmp/a.png")));

    let source: TauriMediaSource = serde_json::from_str(r#"{"kind":"bytes","bytes":[1,2,3]}"#).unwrap();
    assert!(matches!(&source, TauriMediaSource::Bytes { bytes, file_name: None } if bytes == &vec![1, 2, 3]));

    let source: TauriMediaSource = serde_json::from_str(r#"{"kind":"bytes","bytes":[1],"file_name":"a.png"}"#).unwrap();
    assert!(matches!(&source, TauriMediaSource::Bytes { file_name: Some(name), .. } if name == "a.png"));
  }

  #[test]
  fn debug_never_dumps_bytes() {
    let source = TauriMediaSource::Bytes { bytes: vec![0; 100_000], file_name: Some("a.png".into()) };
    let debug = format!("{source:?}");
    assert!(debug.contains("<100000 bytes>"));
    assert!(debug.len() < 200);
  }

  #[test]
  fn validation() {
    assert!(TauriMediaSource::MediaFileToken { token: MediaFileToken::new_from_str("m_a") }.validate().is_ok());
    assert!(TauriMediaSource::Bytes { bytes: vec![1], file_name: None }.validate().is_ok());
    assert!(matches!(
      TauriMediaSource::Bytes { bytes: vec![], file_name: None }.validate(),
      Err(GenerateError::BadInput(BadInputReason::EmptyMediaBytes)),
    ));
    assert!(matches!(
      TauriMediaSource::LocalPath { path: PathBuf::from("/definitely/not/real.png") }.validate(),
      Err(GenerateError::BadInput(BadInputReason::LocalMediaFileNotFound { .. })),
    ));
  }

  #[test]
  fn legacy_tokens_fold_into_sources() {
    let token = MediaFileToken::new_from_str("m_x");
    let merged = merge_source_with_legacy_token(None, Some(token.clone()));
    assert!(matches!(merged, Some(TauriMediaSource::MediaFileToken { .. })));

    // A source field wins over its legacy twin.
    let merged = merge_source_with_legacy_token(
      Some(TauriMediaSource::LocalPath { path: PathBuf::from("/tmp/a.png") }),
      Some(token.clone()),
    );
    assert!(matches!(merged, Some(TauriMediaSource::LocalPath { .. })));

    assert!(merge_sources_with_legacy_tokens(None, Some(vec![])).is_none());
    let merged = merge_sources_with_legacy_tokens(None, Some(vec![token])).unwrap();
    assert_eq!(merged.len(), 1);
  }
}
