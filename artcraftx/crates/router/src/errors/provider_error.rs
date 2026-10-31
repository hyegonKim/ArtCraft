use std::error::Error;
use std::fmt::{Display, Formatter};
use artcraft_client::error::storyteller_error::StorytellerError;
use fal_client::error::fal_error_plus::FalErrorPlus;
use gmicloud_client::error::gmicloud_error::GmiCloudError;
use grok_api_client::error::grok_error::GrokError;
use higgsfield_client::error::higgsfield_error::HiggsfieldError;
use midjourney_client::error::midjourney_error::MidjourneyError;
use seedance2pro_client::error::seedance2pro_error::Seedance2ProError;

#[derive(Debug)]
pub enum ProviderError {
  Storyteller(StorytellerError),
  Fal(FalErrorPlus),
  GmiCloud(GmiCloudError),
  /// API-key (xAI) Grok client failed.
  GrokApi(GrokError),
  Midjourney(MidjourneyError),
  /// First-party (cookie-session) Grok Imagine failed (websocket/POST).
  Grok(String),
  /// First-party (cookie-session) Higgsfield failed. Check
  /// `needs_browser_reauth()` on the inner error to know whether the user has
  /// to log in again.
  Higgsfield(HiggsfieldError),
  /// First-party Midjourney rejected the submit because the account has no
  /// active paid subscription. First-class so callers can prompt to subscribe.
  MidjourneySubscriptionRequired(String),
  /// First-party Midjourney submit returned no job id for some other reason
  /// (e.g. softban, filtered prompt). The string carries the raw detail.
  MidjourneySubmitRejected(String),
  Seedance2Pro(Seedance2ProError),
}

impl Error for ProviderError {}

impl Display for ProviderError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Storyteller(e) => write!(f, "Storyteller provider error: {}", e),
      Self::Fal(e) => write!(f, "Fal provider error: {}", e),
      Self::GmiCloud(e) => write!(f, "GmiCloud provider error: {}", e),
      Self::GrokApi(e) => write!(f, "Grok API provider error: {}", e),
      Self::Midjourney(e) => write!(f, "Midjourney provider error: {}", e),
      Self::Grok(detail) => write!(f, "Grok provider error: {}", detail),
      Self::Higgsfield(e) => write!(f, "Higgsfield provider error: {}", e),
      Self::MidjourneySubscriptionRequired(detail) => write!(f, "Midjourney subscription required: {}", detail),
      Self::MidjourneySubmitRejected(detail) => write!(f, "Midjourney submit rejected: {}", detail),
      Self::Seedance2Pro(e) => write!(f, "Seedance2Pro provider error: {}", e),
    }
  }
}

impl From<StorytellerError> for ProviderError {
  fn from(error: StorytellerError) -> Self {
    Self::Storyteller(error)
  }
}

impl From<FalErrorPlus> for ProviderError {
  fn from(error: FalErrorPlus) -> Self {
    Self::Fal(error)
  }
}

impl From<GmiCloudError> for ProviderError {
  fn from(error: GmiCloudError) -> Self {
    Self::GmiCloud(error)
  }
}

impl From<GrokError> for ProviderError {
  fn from(error: GrokError) -> Self {
    Self::GrokApi(error)
  }
}

impl From<HiggsfieldError> for ProviderError {
  fn from(error: HiggsfieldError) -> Self {
    Self::Higgsfield(error)
  }
}

impl From<MidjourneyError> for ProviderError {
  fn from(error: MidjourneyError) -> Self {
    Self::Midjourney(error)
  }
}

impl From<Seedance2ProError> for ProviderError {
  fn from(error: Seedance2ProError) -> Self {
    Self::Seedance2Pro(error)
  }
}
