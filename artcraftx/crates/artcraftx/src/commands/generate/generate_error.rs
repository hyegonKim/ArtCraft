use crate::error::artcraftx_error::ArtcraftXError;
use anyhow::anyhow;
use router::errors::artcraft_router_error::ArtcraftRouterError;
use router::errors::provider_error::ProviderError;
use base64::DecodeError;
use core_types::enums::generation_source::GenerationSource;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use errors::AnyhowError;
use grok_consumer_client::error::grok_error::GrokError;
use higgsfield_client::error::higgsfield_error::HiggsfieldError;
use midjourney_client::error::midjourney_error::MidjourneyError;
use artcraft_client::error::storyteller_error::StorytellerError;
use router::errors::download_error::DownloadError;
//use fal_client::error::fal_error_plus::FalErrorPlus;

#[derive(Debug)]
pub enum GenerateError {
  BadInput(BadInputReason),
  MissingCredentials(MissingCredentialsReason),
  /// Problem with the stored credential named by the request
  /// (`credential_id`): absent, unknown, or unusable.
  CredentialProblem(CredentialProblemReason),
  ProviderFailure(ProviderFailureReason),

  /// We couldn't find a provider to dispatch the request to.
  NoProviderAvailable,
  
  /// We pulled out Fal (for now) - it's impacting build speeds. We'll add it in the future.
  FalNoLongerSupported,
  
  /// Wrong provider for the model
  BadProviderForModel {
    provider: GenerationSource,
    model: CommonModelType,
  },

  /// If the response didn't contain job tokens to track.
  ResponseHadNoJobTokens,

  /// Issue with Artcraft router downloads
  ArtcraftRouterDownloadError(DownloadError),

  /// A provider we don't support yet in ArtCraft "the Tauri app" part.
  ArtcraftRouterNotYetSupportedProvider(&'static str),

  /// There was a billing, credits, or payments issue.
  BillingIssue(BillingIssueReason),

  /// The feature is not yet implemented.
  NotYetImplemented(String),

  /// The provider accepted the request but rejected it with a user-facing
  /// message (e.g. Midjourney "subscription_required", a banned prompt).
  ProviderRejected(String),

  // Misc error buckets
  AnyhowError(AnyhowError),
  DecodeError(DecodeError),
  IoError(std::io::Error),
}

#[derive(Debug)]
pub enum BadInputReason {
  Base64DecodeError,
  BothImageMaskMediaTokenAndBytesSupplied,
  CannotDetermineImageMimeType,
  /// A `local_path` media source doesn't point at an existing regular file.
  LocalMediaFileNotFound { path: std::path::PathBuf },
  /// A `bytes` media source arrived empty.
  EmptyMediaBytes,
  InvalidNumberOfInputImages {
    provided: u32,
    min: u32,
    max: u32,
  },
  InvalidNumberOfRequestedImages {
    requested: u32,
    min: u32,
    max: u32,
  },
  NoModelSpecified,
  RequiredSourceImageMaskNotProvided,
  RequiredSourceImageNotProvided,
  WrongImageArguments(String),
}

#[derive(Debug)]
pub enum CredentialProblemReason {
  /// The request didn't name a credential (no account selected).
  NoCredentialSupplied,
  /// The named credential doesn't exist on disk.
  CredentialNotFound { credential_id: String },
  /// The credential exists but can't serve this request.
  CredentialNotUsable { credential_id: String, reason: String },
  /// The provider rejected the credential's session; only a fresh browser
  /// login fixes it. The credential file is marked (see
  /// `CookieCredential::relogin_required_since`) so nothing retries the API
  /// until the user logs in again.
  SessionExpired { credential_id: String, service: GenerationSource },
}

impl CredentialProblemReason {
  /// What to tell the user. Shared by the generate commands' error responses
  /// and the credential-error modal so both say the same thing.
  pub fn user_message(&self) -> String {
    match self {
      Self::NoCredentialSupplied => {
        "No account selected. Pick an account in the account selector, \
         or add one in Settings → Accounts.".to_string()
      }
      Self::CredentialNotFound { credential_id } => format!(
        "The selected account no longer exists (credential {}). \
         Pick another account and try again.",
        credential_id,
      ),
      Self::CredentialNotUsable { reason, .. } => {
        format!("The selected account can't be used for this request: {}", reason)
      }
      Self::SessionExpired { service, .. } => format!(
        "Your session has expired and {} needs you to login again.",
        service_display_name(*service),
      ),
    }
  }
}

/// The provider name as users know it, for messages.
fn service_display_name(service: GenerationSource) -> &'static str {
  match service {
    GenerationSource::Higgsfield | GenerationSource::HiggsfieldCookies => "Higgsfield",
    GenerationSource::Grok | GenerationSource::GrokCookies | GenerationSource::XAiApi => "Grok",
    GenerationSource::Midjourney | GenerationSource::MidjourneyCookies => "Midjourney",
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies
    | GenerationSource::ArtcraftApi => "ArtCraft",
    GenerationSource::Fal | GenerationSource::FalApi => "Fal",
    GenerationSource::MagnificCookies => "Magnific",
    GenerationSource::OpenArtCookies => "OpenArt",
    GenerationSource::RunwayCookies => "Runway",
    GenerationSource::OpenAiApi => "OpenAI",
    GenerationSource::ReplicateApi => "Replicate",
    GenerationSource::WorldLabs | GenerationSource::WorldLabsCookies => "World Labs",
  }
}

#[derive(Debug)]
pub enum MissingCredentialsReason {
  NeedsGrokCredentials,
  NeedsFalApiKey,
  NeedsMidjourneyCredentials,
  NeedsMidjourneyUserId,
  NeedsMidjourneyUserInfo,
  NeedsStorytellerCredentials,
}

#[derive(Debug)]
pub struct BillingIssueReason {
  pub provider: BillingProvider,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BillingProvider {
  Artcraft,
  Fal,
  Higgsfield,
  Kinovi,
  Midjourney,
}

#[derive(Debug)]
pub enum ProviderFailureReason {
  GrokError(GrokError),
  /// NB: The Grok client doesn't say why certain errors (eg. missing fields) happen, so we synthesize this.
  GrokJobEnqueueFailed,
  //Fal(FalErrorPlus),
  /// First-party Higgsfield failed. `needs_browser_reauth()` on the inner
  /// error means the user has to log into Higgsfield again.
  HiggsfieldError(HiggsfieldError),
  MidjourneyError(MidjourneyError),
  /// NB: The midjourney client doesn't categorize all errors, so we have to do so on our end.
  MidjourneyJobEnqueueFailed,
  StorytellerError(StorytellerError),
}

impl GenerateError {

  //
  // Bad input
  //

  pub fn both_image_mask_media_token_and_bytes_supplied() -> Self {
    Self::BadInput(BadInputReason::BothImageMaskMediaTokenAndBytesSupplied)
  }

  pub fn no_model_specified() -> Self {
    Self::BadInput(BadInputReason::NoModelSpecified)
  }

  pub fn required_source_image_mask_not_provided() -> Self {
    Self::BadInput(BadInputReason::RequiredSourceImageMaskNotProvided)
  }

  pub fn required_source_image_not_provided() -> Self {
    Self::BadInput(BadInputReason::RequiredSourceImageNotProvided)
  }

  //
  // Missing credentials
  //

  pub fn needs_fal_api_key() -> Self {
    Self::MissingCredentials(MissingCredentialsReason::NeedsFalApiKey)
  }

  pub fn needs_grok_credentials() -> Self {
    Self::MissingCredentials(MissingCredentialsReason::NeedsGrokCredentials)
  }

  pub fn needs_midjourney_credentials() -> Self {
    Self::MissingCredentials(MissingCredentialsReason::NeedsMidjourneyCredentials)
  }

  pub fn needs_storyteller_credentials() -> Self {
    Self::MissingCredentials(MissingCredentialsReason::NeedsStorytellerCredentials)
  }
}

impl From<AnyhowError> for GenerateError {
  fn from(value: AnyhowError) -> Self {
    Self::AnyhowError(value)
  }
}

impl From<ArtcraftXError> for GenerateError {
  fn from(value: ArtcraftXError) -> Self {
    match value {
      ArtcraftXError::AnyhowError(e) => Self::AnyhowError(e),
      ArtcraftXError::CredentialError(e) => Self::AnyhowError(anyhow!("AuthCredential error: {}", e)),
      ArtcraftXError::DecodeError(e) => Self::DecodeError(e),
      ArtcraftXError::IoError(e) => Self::IoError(e),
      ArtcraftXError::GrokError(e) => Self::ProviderFailure(ProviderFailureReason::GrokError(e)),
      ArtcraftXError::StorytellerError(e) => Self::ProviderFailure(ProviderFailureReason::StorytellerError(e)),
      ArtcraftXError::RwLockReadError => Self::AnyhowError(anyhow!("Lock read error")),
      ArtcraftXError::RwLockWriteError => Self::AnyhowError(anyhow!("Lock write error")),
      ArtcraftXError::MutexLockError => Self::AnyhowError(anyhow!("Mutex lock error")),
      ArtcraftXError::ReqwestError(err) => Self::AnyhowError(anyhow!("Reqwest error: {:?}", err)),
      ArtcraftXError::BadDownloadFilename { path } => Self::AnyhowError(anyhow!("Bad download file name: {:?}", path)),
      ArtcraftXError::CannotDownloadFilePathAlreadyExists { path } => Self::AnyhowError(anyhow!("Cannot download already existing file: {:?}", path)),
    }
  }
}

//impl From<FalErrorPlus> for GenerateError {
//  fn from(value: FalErrorPlus) -> Self {
//    Self::ProviderFailure(ProviderFailureReason::Fal(value))
//  }
//}

impl From<GrokError> for GenerateError {
  fn from(value: GrokError) -> Self {
    Self::ProviderFailure(ProviderFailureReason::GrokError(value))
  }
}

impl From<HiggsfieldError> for GenerateError {
  fn from(value: HiggsfieldError) -> Self {
    Self::ProviderFailure(ProviderFailureReason::HiggsfieldError(value))
  }
}

impl From<MidjourneyError> for GenerateError {
  fn from(value: MidjourneyError) -> Self {
    Self::ProviderFailure(ProviderFailureReason::MidjourneyError(value))
  }
}

impl From<StorytellerError> for GenerateError {
  fn from(value: StorytellerError) -> Self {
    Self::ProviderFailure(ProviderFailureReason::StorytellerError(value))
  }
}

impl From<ArtcraftRouterError> for GenerateError {
  fn from(value: ArtcraftRouterError) -> Self {
    match value {
      ArtcraftRouterError::Client(e) => Self::BadInput(BadInputReason::WrongImageArguments(e.to_string())),
      ArtcraftRouterError::Download(e) => Self::ArtcraftRouterDownloadError(e),
      ArtcraftRouterError::InvalidInput(msg) => Self::BadInput(BadInputReason::WrongImageArguments(msg)),
      ArtcraftRouterError::ProviderBillingError(err) => {
        let provider = match err {
          ProviderError::Fal(_) => BillingProvider::Fal,
          ProviderError::Seedance2Pro(_) => BillingProvider::Kinovi,
          ProviderError::Midjourney(_)
          | ProviderError::MidjourneySubscriptionRequired(_)
          | ProviderError::MidjourneySubmitRejected(_) => BillingProvider::Midjourney,
          ProviderError::Storyteller(_) => BillingProvider::Artcraft,
          ProviderError::GmiCloud(_) => BillingProvider::Artcraft,
          ProviderError::GrokApi(_) => BillingProvider::Artcraft,
          ProviderError::Grok(_) => BillingProvider::Artcraft,
          ProviderError::Higgsfield(_) => BillingProvider::Higgsfield,
        };
        Self::BillingIssue(BillingIssueReason { provider })
      },
      ArtcraftRouterError::Provider(ProviderError::Storyteller(e)) => Self::ProviderFailure(ProviderFailureReason::StorytellerError(e)),
      ArtcraftRouterError::Provider(ProviderError::Fal(_)) => Self::FalNoLongerSupported,
      ArtcraftRouterError::Provider(ProviderError::GmiCloud(_)) => Self::ArtcraftRouterNotYetSupportedProvider("gmicloud"),
      ArtcraftRouterError::Provider(ProviderError::GrokApi(_)) => Self::ArtcraftRouterNotYetSupportedProvider("grok_api"),
      // First-party (cookie-session) Grok Imagine websocket/POST failure. The string carries the raw detail.
      ArtcraftRouterError::Provider(ProviderError::Grok(message)) => Self::ProviderRejected(message),
      // First-party (cookie-session) Higgsfield.
      ArtcraftRouterError::Provider(ProviderError::Higgsfield(e)) => Self::ProviderFailure(ProviderFailureReason::HiggsfieldError(e)),
      ArtcraftRouterError::Provider(ProviderError::Seedance2Pro(_)) => Self::ArtcraftRouterNotYetSupportedProvider("seedance2pro"),
      ArtcraftRouterError::Provider(ProviderError::Midjourney(e)) => Self::ProviderFailure(ProviderFailureReason::MidjourneyError(e)),
      ArtcraftRouterError::Provider(ProviderError::MidjourneySubscriptionRequired(message)) => Self::ProviderRejected(message),
      ArtcraftRouterError::Provider(ProviderError::MidjourneySubmitRejected(message)) => Self::ProviderRejected(message),
      ArtcraftRouterError::UnsupportedModel(model) => Self::NotYetImplemented(format!("Unsupported model: {}", model)),
      ArtcraftRouterError::UnsupportedProviderAndModelForNewApi(_message) => Self::ArtcraftRouterNotYetSupportedProvider("unsupported model for new router API"),
      ArtcraftRouterError::ProviderResponseInvalid(message) => Self::ProviderRejected(message),
    }
  }
}
