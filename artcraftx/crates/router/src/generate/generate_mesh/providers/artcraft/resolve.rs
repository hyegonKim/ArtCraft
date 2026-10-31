//! Shared resolve helpers for the Artcraft mesh provider.
//!
//! Artcraft only accepts media file tokens — raw URLs are rejected.

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::mesh_ref::MeshRef;
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

pub(super) fn resolve_image_ref(
  image_ref: Option<ImageRef>,
) -> Result<Option<MediaFileToken>, ArtcraftRouterError> {
  match image_ref {
    None => Ok(None),
    Some(ImageRef::MediaFileToken(token)) => Ok(Some(token)),
    Some(ImageRef::Url(_) | ImageRef::LocalPath(_) | ImageRef::Bytes(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
  }
}

pub(super) fn resolve_mesh_ref(
  mesh_ref: Option<MeshRef>,
) -> Result<Option<MediaFileToken>, ArtcraftRouterError> {
  match mesh_ref {
    None => Ok(None),
    Some(MeshRef::MediaFileToken(token)) => Ok(Some(token)),
    Some(MeshRef::Url(_) | MeshRef::LocalPath(_) | MeshRef::Bytes(_)) => {
      Err(ArtcraftRouterError::Client(ClientError::ArtcraftOnlySupportsMediaTokens))
    }
  }
}
