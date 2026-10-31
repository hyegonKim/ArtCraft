//! Per-provider conversion of [`TauriMediaSource`]s.
//!
//! - Higgsfield / Kinovi consume sources natively (`sources_to_*_list`); the
//!   router reads local files / bytes and uploads them straight to the
//!   provider. Only tokens (cloud-library picks) resolve through ArtCraft.
//! - The ArtCraft provider is token-native: local files / bytes upload to
//!   ArtCraft at generate time (`sources_to_artcraft_tokens`) — the media
//!   must reach ArtCraft anyway.
//! - FAL needs public URLs and has no storage of its own: local files /
//!   bytes upload to ArtCraft and resolve to CDN URLs
//!   (`image_sources_to_fal_urls`). FAL is the ONLY non-ArtCraft provider
//!   allowed to route local media through the ArtCraft cloud.

use std::path::{Path, PathBuf};

use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use artcraft_client::endpoints::media_files::upload_audio_media_file_from_file::{upload_audio_media_file_from_file, UploadAudioFromFileArgs};
use artcraft_client::endpoints::media_files::upload_image_media_file_from_file::{upload_image_media_file_from_file, UploadImageFromFileArgs};
use artcraft_client::endpoints::media_files::upload_video_media_file_from_file::{upload_video_media_file_from_file, UploadVideoFromFileArgs};
use artcraft_client::error::storyteller_error::StorytellerError;
use artcraft_client::utils::api_host::ApiHost;
use log::info;
use router::api::audio_list_ref::AudioListRef;
use router::api::image_list_ref::ImageListRef;
use router::api::image_ref::ImageRef;
use router::api::video_list_ref::VideoListRef;
use router::utils::higgsfield_media::sniff_mime_type;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use uuid_utils::uuid::generate_random_uuid;

use crate::commands::generate::common::tauri_media_source::TauriMediaSource;
use crate::commands::generate::generate_error::GenerateError;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;

/// Which ArtCraft upload endpoint a source goes through.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArtcraftMediaKind {
  Image,
  Video,
  Audio,
}

// ── Router-native providers (Higgsfield, Kinovi) ──

pub fn source_to_image_ref(maybe_source: Option<TauriMediaSource>) -> Option<ImageRef> {
  maybe_source.map(TauriMediaSource::into_image_ref)
}

pub fn sources_to_image_list(maybe_sources: Option<Vec<TauriMediaSource>>) -> Option<ImageListRef> {
  maybe_sources.map(|sources| {
    ImageListRef::Sources(sources.into_iter().map(TauriMediaSource::into_image_ref).collect())
  })
}

pub fn sources_to_video_list(maybe_sources: Option<Vec<TauriMediaSource>>) -> Option<VideoListRef> {
  maybe_sources.map(|sources| {
    VideoListRef::Sources(sources.into_iter().map(TauriMediaSource::into_video_ref).collect())
  })
}

pub fn sources_to_audio_list(maybe_sources: Option<Vec<TauriMediaSource>>) -> Option<AudioListRef> {
  maybe_sources.map(|sources| {
    AudioListRef::Sources(sources.into_iter().map(TauriMediaSource::into_audio_ref).collect())
  })
}

/// The ArtCraft media tokens among `sources`, deduplicated, in order — the
/// only sources that need the token→CDN-URL map.
pub fn collect_source_tokens<'a>(sources: impl IntoIterator<Item = &'a TauriMediaSource>) -> Vec<MediaFileToken> {
  let mut tokens: Vec<MediaFileToken> = sources.into_iter()
      .filter_map(TauriMediaSource::maybe_media_file_token)
      .cloned()
      .collect();
  tokens.dedup();
  tokens
}

// ── ArtCraft provider (token-native) ──

/// Resolve one source to an ArtCraft media token: tokens pass through;
/// local files and bytes upload to ArtCraft at generate time — unless the
/// ledger says this account already uploaded the same content and ArtCraft
/// confirms it still has it.
pub async fn source_to_artcraft_token(
  source: TauriMediaSource,
  kind: ArtcraftMediaKind,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
  ledger: &AssetUploadLedger,
) -> Result<MediaFileToken, GenerateError> {
  let maybe_cache = ledger.for_artcraft_session(maybe_creds);
  match source {
    TauriMediaSource::MediaFileToken { token } => Ok(token),
    TauriMediaSource::LocalPath { path } => match &maybe_cache {
      Some(cache) => cache.artcraft_token_for_file(api_host, &path, || upload_path_to_artcraft(&path, kind, maybe_creds, api_host)).await,
      None => upload_path_to_artcraft(&path, kind, maybe_creds, api_host).await,
    },
    TauriMediaSource::Bytes { bytes, file_name } => {
      let upload = || upload_bytes_to_artcraft(&bytes, file_name.as_deref(), kind, maybe_creds, api_host);
      match &maybe_cache {
        Some(cache) => cache.artcraft_token_for_bytes(api_host, &bytes, upload).await,
        None => upload().await,
      }
    }
  }
}

/// List twin of [`source_to_artcraft_token`]; order is preserved.
pub async fn sources_to_artcraft_tokens(
  maybe_sources: Option<Vec<TauriMediaSource>>,
  kind: ArtcraftMediaKind,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
  ledger: &AssetUploadLedger,
) -> Result<Option<Vec<MediaFileToken>>, GenerateError> {
  let sources = match maybe_sources {
    None => return Ok(None),
    Some(sources) if sources.is_empty() => return Ok(None),
    Some(sources) => sources,
  };
  let mut tokens = Vec::with_capacity(sources.len());
  for source in sources {
    tokens.push(source_to_artcraft_token(source, kind, maybe_creds, api_host, ledger).await?);
  }
  Ok(Some(tokens))
}

/// Single-optional twin of [`source_to_artcraft_token`].
pub async fn maybe_source_to_artcraft_token(
  maybe_source: Option<TauriMediaSource>,
  kind: ArtcraftMediaKind,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
  ledger: &AssetUploadLedger,
) -> Result<Option<MediaFileToken>, GenerateError> {
  match maybe_source {
    None => Ok(None),
    Some(source) => Ok(Some(source_to_artcraft_token(source, kind, maybe_creds, api_host, ledger).await?)),
  }
}

// ── FAL (URL-native; the ONLY provider allowed to route through ArtCraft) ──

/// Resolve image sources to publicly-reachable CDN URLs for FAL: tokens
/// resolve to their CDN URL; local files / bytes upload to ArtCraft first.
pub async fn image_sources_to_fal_urls(
  maybe_sources: Option<Vec<TauriMediaSource>>,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
  ledger: &AssetUploadLedger,
) -> Result<Option<Vec<String>>, GenerateError> {
  let sources = match maybe_sources {
    None => return Ok(None),
    Some(sources) if sources.is_empty() => return Ok(None),
    Some(sources) => sources,
  };
  let mut urls = Vec::with_capacity(sources.len());
  for source in sources {
    let token = source_to_artcraft_token(source, ArtcraftMediaKind::Image, maybe_creds, api_host, ledger).await?;
    let response = get_media_file(api_host, &token).await?;
    urls.push(response.media_file.media_links.cdn_url.to_string());
  }
  Ok(Some(urls))
}

// ── Private ──

/// The upload endpoints are multipart-from-file; stage the bytes, upload,
/// clean up.
async fn upload_bytes_to_artcraft(
  bytes: &[u8],
  maybe_file_name: Option<&str>,
  kind: ArtcraftMediaKind,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
) -> Result<MediaFileToken, GenerateError> {
  let staged = stage_bytes_to_temp_file(bytes, maybe_file_name)?;
  let result = upload_path_to_artcraft(&staged, kind, maybe_creds, api_host).await;
  std::fs::remove_file(&staged).ok();
  result
}

async fn upload_path_to_artcraft(
  path: &Path,
  kind: ArtcraftMediaKind,
  maybe_creds: Option<&StorytellerCredentialSet>,
  api_host: &ApiHost,
) -> Result<MediaFileToken, GenerateError> {
  info!("Uploading local {:?} file to ArtCraft at generate time: {}", kind, path.display());
  let token = match kind {
    ArtcraftMediaKind::Image => {
      upload_image_media_file_from_file(UploadImageFromFileArgs {
        api_host,
        maybe_creds,
        path,
        is_intermediate_system_file: false,
        maybe_prompt_token: None,
        maybe_generation_provider: None,
        maybe_batch_token: None,
      }).await?.media_file_token
    }
    ArtcraftMediaKind::Video => {
      upload_video_media_file_from_file(UploadVideoFromFileArgs {
        api_host,
        maybe_creds,
        path,
        maybe_prompt_token: None,
        maybe_generation_provider: None,
      }).await.map_err(StorytellerError::from)?.media_file_token
    }
    ArtcraftMediaKind::Audio => {
      upload_audio_media_file_from_file(UploadAudioFromFileArgs {
        api_host,
        maybe_creds,
        path,
        maybe_prompt_token: None,
        maybe_generation_provider: None,
      }).await.map_err(StorytellerError::from)?.media_file_token
    }
  };
  info!("Uploaded local {:?} file as {}", kind, token.as_str());
  Ok(token)
}

/// Stage in-memory bytes as a temp file for the multipart-from-file upload
/// endpoints. The file name's extension survives so the server can tell the
/// type; unknown types get one sniffed from the bytes.
fn stage_bytes_to_temp_file(bytes: &[u8], maybe_file_name: Option<&str>) -> Result<PathBuf, GenerateError> {
  let extension = maybe_file_name
      .and_then(|name| Path::new(name).extension()?.to_str().map(str::to_string))
      .or_else(|| sniff_mime_type(bytes).map(|mime| mime.file_extension().to_string()))
      .unwrap_or_else(|| "bin".to_string());
  let path = std::env::temp_dir().join(format!("artcraftx_media_{}.{}", generate_random_uuid(), extension));
  std::fs::write(&path, bytes).map_err(GenerateError::IoError)?;
  Ok(path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sources_map_to_router_lists() {
    let list = sources_to_image_list(Some(vec![
      TauriMediaSource::MediaFileToken { token: MediaFileToken::new_from_str("m_a") },
      TauriMediaSource::LocalPath { path: PathBuf::from("/tmp/a.png") },
    ])).unwrap();
    assert_eq!(list.len(), 2);
    assert!(matches!(&list, ImageListRef::Sources(refs) if matches!(refs[1], ImageRef::LocalPath(_))));
    assert!(sources_to_image_list(None).is_none());
  }

  #[test]
  fn only_tokens_are_collected_for_the_url_map() {
    let sources = vec![
      TauriMediaSource::MediaFileToken { token: MediaFileToken::new_from_str("m_a") },
      TauriMediaSource::LocalPath { path: PathBuf::from("/tmp/a.png") },
      TauriMediaSource::Bytes { bytes: vec![1], file_name: None },
      TauriMediaSource::MediaFileToken { token: MediaFileToken::new_from_str("m_b") },
    ];
    let tokens = collect_source_tokens(&sources);
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].as_str(), "m_a");
    assert_eq!(tokens[1].as_str(), "m_b");
  }

  #[test]
  fn byte_staging_names_by_extension() {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    let staged = stage_bytes_to_temp_file(PNG, None).unwrap();
    assert_eq!(staged.extension().and_then(|e| e.to_str()), Some("png"));
    assert_eq!(std::fs::read(&staged).unwrap(), PNG);
    std::fs::remove_file(&staged).ok();

    let staged = stage_bytes_to_temp_file(b"????", Some("track.mp3")).unwrap();
    assert_eq!(staged.extension().and_then(|e| e.to_str()), Some("mp3"));
    std::fs::remove_file(&staged).ok();
  }
}
