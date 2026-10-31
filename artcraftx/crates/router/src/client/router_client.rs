use crate::client::multi_router_client::MultiRouterClient;
use crate::client::router_artcraft_client::RouterArtcraftClient;
use crate::client::router_fal_client::RouterFalClient;
use crate::client::router_gmicloud_client::RouterGmiCloudClient;
use crate::client::router_grok_api_client::RouterGrokApiClient;
use crate::client::router_grok_client::RouterGrokClient;
use crate::client::router_higgsfield_client::RouterHiggsfieldClient;
use crate::client::router_midjourney_client::RouterMidjourneyClient;
use crate::client::router_seedance2pro_client::RouterSeedance2ProClient;
use crate::errors::client_error::{ClientError, ClientType};

pub enum RouterClient {
  Multi(MultiRouterClient),
  Artcraft(RouterArtcraftClient),
  Fal(RouterFalClient),
  GmiCloud(RouterGmiCloudClient),
  GrokApi(RouterGrokApiClient),
  Grok(RouterGrokClient),
  Higgsfield(RouterHiggsfieldClient),
  Midjourney(RouterMidjourneyClient),
  Seedance2Pro(RouterSeedance2ProClient),
}

impl RouterClient {
  pub fn get_artcraft_client_ref(&self) -> Result<&RouterArtcraftClient, ClientError> {
    match self {
      RouterClient::Artcraft(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_artcraft_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Artcraft)),
    }
  }

  pub fn get_fal_client_ref(&self) -> Result<&RouterFalClient, ClientError> {
    match self {
      RouterClient::Fal(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_fal_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Fal)),
    }
  }

  pub fn get_gmicloud_client_ref(&self) -> Result<&RouterGmiCloudClient, ClientError> {
    match self {
      RouterClient::GmiCloud(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_gmicloud_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::GmiCloud)),
    }
  }

  pub fn get_grok_api_client_ref(&self) -> Result<&RouterGrokApiClient, ClientError> {
    match self {
      RouterClient::GrokApi(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_grok_api_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::GrokApi)),
    }
  }

  pub fn get_grok_client_ref(&self) -> Result<&RouterGrokClient, ClientError> {
    match self {
      RouterClient::Grok(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_grok_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Grok)),
    }
  }

  pub fn get_higgsfield_client_ref(&self) -> Result<&RouterHiggsfieldClient, ClientError> {
    match self {
      RouterClient::Higgsfield(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_higgsfield_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Higgsfield)),
    }
  }

  pub fn get_midjourney_client_ref(&self) -> Result<&RouterMidjourneyClient, ClientError> {
    match self {
      RouterClient::Midjourney(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_midjourney_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Midjourney)),
    }
  }

  pub fn get_seedance2pro_client_ref(&self) -> Result<&RouterSeedance2ProClient, ClientError> {
    match self {
      RouterClient::Seedance2Pro(client) => Ok(client),
      RouterClient::Multi(multi) => multi.get_seedance2pro_client_ref(),
      _ => Err(ClientError::ClientNotConfigured(ClientType::Seedance2Pro)),
    }
  }

}
