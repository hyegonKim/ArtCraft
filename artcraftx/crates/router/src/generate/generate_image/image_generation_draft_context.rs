use std::collections::HashMap;
use std::fmt::Debug;

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::asset_upload_cache::AssetUploadCache;
use crate::api::asset_upload_observer::AssetUploadObserver;
use crate::client::router_client::RouterClient;
use crate::client::router_higgsfield_client::RouterHiggsfieldClient;
use crate::client::router_seedance2pro_client::RouterSeedance2ProClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;

/// Context passed when finalizing an image draft into a concrete request.
///
/// Drafts only exist for providers that need to upload assets before
/// sending the request: the Seedance2Pro/Kinovi pipeline (used for
/// Midjourney image generation with reference images) and first-party
/// Higgsfield (reference images become Higgsfield media ids); other
/// providers — Artcraft, Fal — accept image URLs directly and skip the
/// draft phase entirely.
#[derive(Clone, Default)]
pub struct ImageGenerationDraftContext<'a> {
  /// Optional: the router client, needed for providers that require
  /// authenticated calls (e.g. Seedance2Pro/Kinovi uploads).
  pub client: Option<&'a RouterClient>,

  /// Optional: a map of Media File Tokens to their ArtCraft URLs.
  /// Required only when the draft has unresolved `MediaFileToken` image
  /// inputs that must be downloaded from ArtCraft and re-uploaded to the
  /// target provider's CDN.
  pub media_file_to_artcraft_url_map: Option<&'a HashMap<MediaFileToken, String>>,

  /// Optional: the account's upload history on the target provider, so
  /// media it already holds isn't uploaded again.
  pub asset_upload_cache: Option<&'a dyn AssetUploadCache>,

  /// Optional: told whether each reference was reused or uploaded, so the
  /// caller can show the right notice.
  pub asset_upload_observer: Option<&'a dyn AssetUploadObserver>,
}

impl<'a> ImageGenerationDraftContext<'a> {
  pub fn get_seedance2pro_client_ref(&self) -> Result<&RouterSeedance2ProClient, ArtcraftRouterError> {
    let client = self.client.ok_or(ArtcraftRouterError::Client(ClientError::RouterClientNotProvided))?;
    client.get_seedance2pro_client_ref()
      .map_err(ArtcraftRouterError::Client)
  }

  pub fn get_higgsfield_client_ref(&self) -> Result<&RouterHiggsfieldClient, ArtcraftRouterError> {
    let client = self.client.ok_or(ArtcraftRouterError::Client(ClientError::RouterClientNotProvided))?;
    client.get_higgsfield_client_ref()
      .map_err(ArtcraftRouterError::Client)
  }
}

impl Debug for ImageGenerationDraftContext<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ImageGenerationDraftContext")
      .field("client", &self.client.is_some())
      .field("media_file_to_artcraft_url_map", &self.media_file_to_artcraft_url_map.map(|m| m.len()))
      .field("asset_upload_cache", &self.asset_upload_cache.is_some())
      .field("asset_upload_observer", &self.asset_upload_observer.is_some())
      .finish()
  }
}
