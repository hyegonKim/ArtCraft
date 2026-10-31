//! Shared resolve helpers for the Artcraft audio provider.
//!
//! Artcraft only accepts media file tokens — raw URLs are rejected.

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::audio_list_ref::AudioListRef;
use crate::api::audio_ref::AudioRef;
use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;

pub(super) fn resolve_audio_list_ref(
  audio_list_ref: Option<AudioListRef>,
) -> Result<Option<Vec<MediaFileToken>>, ArtcraftRouterError> {
  match audio_list_ref {
    None => Ok(None),
    Some(AudioListRef::MediaFileTokens(tokens)) => Ok(Some(tokens)),
    Some(AudioListRef::Urls(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
    Some(AudioListRef::Sources(refs)) => {
      let mut tokens = Vec::with_capacity(refs.len());
      for audio_ref in refs {
        match audio_ref {
          AudioRef::MediaFileToken(token) => tokens.push(token),
          _ => return Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens)),
        }
      }
      Ok(Some(tokens))
    }
  }
}

pub(super) fn resolve_image_list_ref(
  image_list_ref: Option<ImageListRef>,
) -> Result<Option<Vec<MediaFileToken>>, ArtcraftRouterError> {
  match image_list_ref {
    None => Ok(None),
    Some(ImageListRef::MediaFileTokens(tokens)) => Ok(Some(tokens)),
    Some(ImageListRef::Urls(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
    Some(ImageListRef::Sources(refs)) => {
      let mut tokens = Vec::with_capacity(refs.len());
      for image_ref in refs {
        match image_ref {
          ImageRef::MediaFileToken(token) => tokens.push(token),
          _ => return Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens)),
        }
      }
      Ok(Some(tokens))
    }
  }
}
