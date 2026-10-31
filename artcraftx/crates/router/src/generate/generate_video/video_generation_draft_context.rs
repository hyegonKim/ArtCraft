use crate::api::asset_upload_cache::AssetUploadCache;
use crate::api::asset_upload_observer::AssetUploadObserver;
use crate::client::router_client::RouterClient;
use crate::client::router_higgsfield_client::RouterHiggsfieldClient;
use crate::client::router_seedance2pro_client::RouterSeedance2ProClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::client_error::ClientError;
use std::collections::HashMap;
use std::fmt::Debug;
use artcraft_client::tokens::characters::CharacterToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

#[derive(Clone, Default)]
pub struct VideoGenerationDraftContext<'a> {
  /// Optional: the router client, needed for providers that require authentication.
  pub client: Option<&'a RouterClient>,

  /// Optional context: a map of Media File Tokens to their ArtCraft URLs
  /// Only needed if we have to fetch these assets and upload them to another provider.
  pub media_file_to_artcraft_url_map: Option<&'a HashMap<MediaFileToken, String>>,

  /// Optional context: a map of Character Tokens to their respective Kinovi IDs
  /// Only necessary if using Kinovi characters
  pub character_token_to_kinovi_id_map: Option<&'a HashMap<CharacterToken, String>>,

  /// Optional: the account's upload history on the target provider, so
  /// media it already holds isn't uploaded again.
  pub asset_upload_cache: Option<&'a dyn AssetUploadCache>,

  /// Optional: told whether each reference was reused or uploaded, so the
  /// caller can show the right notice.
  pub asset_upload_observer: Option<&'a dyn AssetUploadObserver>,
}

impl <'a> VideoGenerationDraftContext<'a> {
  pub fn get_seedance2pro_client_ref(&self) -> Result<&RouterSeedance2ProClient, ArtcraftRouterError> {
    let client = self.client.ok_or(ArtcraftRouterError::Client(ClientError::RouterClientNotProvided))?;
    client.get_seedance2pro_client_ref()
      .map_err(|err| ArtcraftRouterError::Client(err))
  }

  pub fn get_higgsfield_client_ref(&self) -> Result<&RouterHiggsfieldClient, ArtcraftRouterError> {
    let client = self.client.ok_or(ArtcraftRouterError::Client(ClientError::RouterClientNotProvided))?;
    client.get_higgsfield_client_ref()
      .map_err(ArtcraftRouterError::Client)
  }

  pub fn get_media_file_to_artcraft_url_map(&self) -> Result<&HashMap<MediaFileToken, String>, ArtcraftRouterError> {
    self.media_file_to_artcraft_url_map
      .ok_or_else(|| ArtcraftRouterError::Client(ClientError::MediaFileToUrlMapNotProvided))
  }

  pub fn get_character_token_to_kinovi_map(&self) -> Result<&HashMap<CharacterToken, String>, ArtcraftRouterError> {
    self.character_token_to_kinovi_id_map
      .ok_or_else(|| ArtcraftRouterError::Client(ClientError::CharacterTokenToKinoviCharacterIdNotProvided))
  }
}

impl Debug for VideoGenerationDraftContext<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("VideoGenerationDraftContext")
      .field("client", &self.client.is_some())
      .field("media_file_to_artcraft_url_map", &self.media_file_to_artcraft_url_map)
      .field("character_token_to_kinovi_id_map", &self.character_token_to_kinovi_id_map)
      .field("asset_upload_cache", &self.asset_upload_cache.is_some())
      .field("asset_upload_observer", &self.asset_upload_observer.is_some())
      .finish()
  }
}
