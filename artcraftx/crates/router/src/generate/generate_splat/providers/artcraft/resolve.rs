//! Shared resolve helpers for the Artcraft splat provider.
//!
//! Artcraft only accepts media file tokens — raw URLs are rejected.

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::video_ref::VideoRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;

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

pub(super) fn resolve_video_ref(
  video_ref: Option<VideoRef>,
) -> Result<Option<MediaFileToken>, ArtcraftRouterError> {
  match video_ref {
    None => Ok(None),
    Some(VideoRef::MediaFileToken(token)) => Ok(Some(token)),
    Some(VideoRef::Url(_) | VideoRef::LocalPath(_) | VideoRef::Bytes(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
  }
}
