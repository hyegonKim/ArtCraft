use artcraft_client::api_defs::users::login::{LoginErrorType, LoginRequest};
use artcraft_client::endpoints::users::login::{login, LoginArgs, LoginError};
use chrono::Utc;
use core_types::enums::generation_source::GenerationSource;
use log::{error, info, warn};
use serde_derive::Serialize;
use tauri::{AppHandle, State};

use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::utils::services::artcraft_api_host::maybe_artcraft_api_host_for_service;
use crate::credentials::cookie_credential::CookieCredential;
use crate::credentials::auth_credential::{AuthCredential, CredentialSecret};
use crate::credentials::service_cookie_origin::cookie_origin_for_service;
use cookie_store_wrapper::cookie_store::CookieStore;
use crate::credentials::credential_user_info::CredentialUserInfo;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::refresh_account_state_event::RefreshAccountStateEvent;
use crate::state::data_dir::app_data_root::AppDataRoot;

#[derive(Serialize)]
pub struct ArtcraftLoginResponse {
  pub credential: CredentialPayload,
}

/// Error payload surfaced to the frontend when a login fails.
#[derive(Clone, Debug, Serialize)]
pub struct ArtcraftLoginCommandError {
  pub error_type: ArtcraftLoginErrorType,
  pub message: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ArtcraftLoginErrorType {
  /// Invalid username/email or password.
  #[serde(rename = "invalid_credentials")]
  InvalidCredentials,
  /// The account was created without a password (needs a password reset).
  #[serde(rename = "account_needs_password")]
  AccountNeedsPassword,
  /// The API returned a server error.
  #[serde(rename = "server_error")]
  ServerError,
  /// We couldn't reach the API (network problem, local dev server not
  /// running, unexpected response).
  #[serde(rename = "connection_error")]
  ConnectionError,
  /// The request was malformed (wrong service, empty fields).
  #[serde(rename = "bad_request")]
  BadRequest,
}

/// Log into an ArtCraft account with a username (or email) and password, then
/// store the resulting session as a new credential file.
///
/// `service` selects the environment: `Artcraft` hits production
/// (`api.storyteller.ai`); `ArtcraftLocal` hits a local dev server. Every
/// successful login creates a NEW credential file, so users can keep an
/// unlimited number of accounts for either environment.
#[tauri::command]
pub async fn artcraft_login_command(
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
  service: GenerationSource,
  username_or_email: String,
  password: String,
) -> Result<ArtcraftLoginResponse, ArtcraftLoginCommandError> {
  info!("artcraft_login_command called for service: {}", service);

  let is_password_login_service = matches!(
    service,
    GenerationSource::Artcraft | GenerationSource::ArtcraftLocal,
  );

  let maybe_api_host = maybe_artcraft_api_host_for_service(service)
      .filter(|_| is_password_login_service);

  let Some(api_host) = maybe_api_host else {
    let message = format!("Service {} does not support password login", service);
    error!("{}", message);
    return Err(ArtcraftLoginCommandError {
      error_type: ArtcraftLoginErrorType::BadRequest,
      message,
    });
  };

  let username_or_email = username_or_email.trim().to_string();
  if username_or_email.is_empty() || password.is_empty() {
    return Err(ArtcraftLoginCommandError {
      error_type: ArtcraftLoginErrorType::BadRequest,
      message: "Enter a username (or email) and password".to_string(),
    });
  }

  let request = LoginRequest {
    username_or_email: username_or_email.clone(),
    password,
  };

  let response = login(LoginArgs {
    api_host: &api_host,
    request: &request,
  }).await.map_err(|err| {
    warn!("ArtCraft login failed for service {}: {}", service, err);
    login_error_to_command_error(err)
  })?;

  let credential = save_session_credential(
    &app_data_root,
    service,
    &response.signed_session,
    username_or_email,
  )?;

  RefreshAccountStateEvent {
    provider: Some(GenerationSource::Artcraft),
  }.send_infallible(&app);

  Ok(ArtcraftLoginResponse {
    credential: CredentialPayload::from_auth_credential(&credential),
  })
}

/// Persist the signed session as a NEW cookie credential file (never upserts,
/// so each login adds another account).
fn save_session_credential(
  app_data_root: &AppDataRoot,
  service: GenerationSource,
  signed_session: &str,
  username_or_email: String,
) -> Result<AuthCredential, ArtcraftLoginCommandError> {
  let now = Utc::now();

  let Some(cookie_origin) = cookie_origin_for_service(service) else {
    return Err(ArtcraftLoginCommandError {
      error_type: ArtcraftLoginErrorType::BadRequest,
      message: format!("Service {} has no cookie origin", service),
    });
  };
  let cookie = CookieCredential {
    updated_at: Some(now),
    failed_at: None,
    succeeded_at: Some(now),
    relogin_required_since: None,
    user_agent: None,
    grok_data: None,
    cookies: CookieStore::from_cookie_header(
        &format!("session={}", signed_session), &cookie_origin),
  };

  let user_info = if username_or_email.contains('@') {
    CredentialUserInfo {
      username: None,
      email: Some(username_or_email),
    }
  } else {
    CredentialUserInfo {
      username: Some(username_or_email),
      email: None,
    }
  };

  let creds_dir = app_data_root.credentials_dir();

  let credential = AuthCredential {
    id: creds_dir.generate_unique_credential_id(),
    service,
    name: None,
    secret: CredentialSecret::Cookies(cookie),
    user_info: Some(user_info),
    source_path: creds_dir.next_available_credential_path(service),
  };

  creds_dir.save_credential(&credential).map_err(|err| {
    error!("Error saving login credential: {}", err);
    ArtcraftLoginCommandError {
      error_type: ArtcraftLoginErrorType::ServerError,
      message: format!("Login succeeded but saving the credential failed: {}", err),
    }
  })?;

  Ok(credential)
}

fn login_error_to_command_error(error: LoginError) -> ArtcraftLoginCommandError {
  match error {
    LoginError::Login(response) => {
      let error_type = match response.error_type {
        LoginErrorType::InvalidCredentials => ArtcraftLoginErrorType::InvalidCredentials,
        LoginErrorType::AccountNeedsPassword => ArtcraftLoginErrorType::AccountNeedsPassword,
        LoginErrorType::ServerError => ArtcraftLoginErrorType::ServerError,
      };
      ArtcraftLoginCommandError {
        error_type,
        message: response.error_message,
      }
    }
    LoginError::Storyteller(err) => ArtcraftLoginCommandError {
      error_type: ArtcraftLoginErrorType::ConnectionError,
      message: format!("Could not reach the server: {}", err),
    },
  }
}
