use std::collections::HashMap;

use seedance2pro_client::creds::seedance2pro_session::Seedance2ProSession;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::character_list_ref::CharacterListRef;
use crate::api::image_ref::ImageRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use crate::generate::generate_video::providers::kinovi::upload::upload_source_to_seedance2pro;
use crate::generate::generate_video::video_generation_draft_context::VideoGenerationDraftContext;
use crate::utils::media_source_ref::MediaSourceRef;

/// Resolve a single ImageRef and upload to Seedance2Pro CDN.
pub(crate) async fn resolve_and_upload_single(
  session: &Seedance2ProSession,
  image_ref: Option<ImageRef>,
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
) -> Result<Option<String>, ArtcraftRouterError> {
  match image_ref {
    None => Ok(None),
    Some(image_ref) => Ok(Some(upload_source_to_seedance2pro(session, image_ref.into(), maybe_map).await?)),
  }
}

/// Resolve a list of sources and upload each to Seedance2Pro CDN. Order is
/// preserved. `None` / empty lists resolve to `None`.
pub(crate) async fn resolve_and_upload_list(
  session: &Seedance2ProSession,
  sources: Option<Vec<MediaSourceRef>>,
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
) -> Result<Option<Vec<String>>, ArtcraftRouterError> {
  let sources = match sources {
    None => return Ok(None),
    Some(sources) if sources.is_empty() => return Ok(None),
    Some(sources) => sources,
  };

  let mut uploaded = Vec::with_capacity(sources.len());
  for source in sources {
    uploaded.push(upload_source_to_seedance2pro(session, source, maybe_map).await?);
  }
  Ok(Some(uploaded))
}

/// Map character tokens to their Kinovi character IDs, preserving order.
pub(crate) fn resolve_character_tokens(
  character_list_ref: Option<&CharacterListRef>,
  draft_context: &VideoGenerationDraftContext<'_>,
) -> Result<Option<Vec<String>>, ArtcraftRouterError> {
  let list = match character_list_ref {
    None => return Ok(None),
    Some(r) => r,
  };

  let tokens = match list {
    CharacterListRef::CharacterTokens(tokens) if tokens.is_empty() => return Ok(None),
    CharacterListRef::CharacterTokens(tokens) => tokens,
  };

  let map = draft_context.get_character_token_to_kinovi_map()?;

  let ids: Result<Vec<String>, _> = tokens.iter()
    .map(|token| {
      map.get(token).cloned().ok_or_else(|| {
        ArtcraftRouterError::Client(ClientError::CharacterTokenNotFoundInMap {
          token: token.clone(),
        })
      })
    })
    .collect();

  ids.map(Some)
}
