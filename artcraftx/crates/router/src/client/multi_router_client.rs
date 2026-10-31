use crate::client::router_artcraft_client::RouterArtcraftClient;
use crate::client::router_fal_client::RouterFalClient;
use crate::client::router_gmicloud_client::RouterGmiCloudClient;
use crate::client::router_grok_api_client::RouterGrokApiClient;
use crate::client::router_grok_client::RouterGrokClient;
use crate::client::router_higgsfield_client::RouterHiggsfieldClient;
use crate::client::router_midjourney_client::RouterMidjourneyClient;
use crate::client::router_seedance2pro_client::RouterSeedance2ProClient;
use crate::errors::client_error::{ClientError, ClientType};

pub struct MultiRouterClient {
  pub(crate) artcraft_client: Option<RouterArtcraftClient>,
  pub(crate) fal_client: Option<RouterFalClient>,
  pub(crate) gmicloud_client: Option<RouterGmiCloudClient>,
  pub(crate) grok_api_client: Option<RouterGrokApiClient>,
  pub(crate) grok_client: Option<RouterGrokClient>,
  pub(crate) higgsfield_client: Option<RouterHiggsfieldClient>,
  pub(crate) midjourney_client: Option<RouterMidjourneyClient>,
  pub(crate) seedance2pro_client: Option<RouterSeedance2ProClient>,
}

impl MultiRouterClient {
  pub fn get_artcraft_client_ref(&self) -> Result<&RouterArtcraftClient, ClientError> {
    self.artcraft_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Artcraft))
  }

  pub fn get_fal_client_ref(&self) -> Result<&RouterFalClient, ClientError> {
    self.fal_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Fal))
  }

  pub fn get_gmicloud_client_ref(&self) -> Result<&RouterGmiCloudClient, ClientError> {
    self.gmicloud_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::GmiCloud))
  }

  pub fn get_grok_api_client_ref(&self) -> Result<&RouterGrokApiClient, ClientError> {
    self.grok_api_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::GrokApi))
  }

  pub fn get_grok_client_ref(&self) -> Result<&RouterGrokClient, ClientError> {
    self.grok_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Grok))
  }

  pub fn get_higgsfield_client_ref(&self) -> Result<&RouterHiggsfieldClient, ClientError> {
    self.higgsfield_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Higgsfield))
  }

  pub fn get_midjourney_client_ref(&self) -> Result<&RouterMidjourneyClient, ClientError> {
    self.midjourney_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Midjourney))
  }

  pub fn get_seedance2pro_client_ref(&self) -> Result<&RouterSeedance2ProClient, ClientError> {
    self.seedance2pro_client.as_ref()
      .ok_or(ClientError::ClientNotConfigured(ClientType::Seedance2Pro))
  }

}
