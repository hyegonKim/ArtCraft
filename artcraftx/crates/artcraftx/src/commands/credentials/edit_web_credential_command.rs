use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::credentials::cookie_credential::CookieCredential;
use crate::credentials::auth_credential::CredentialSecret;
use crate::credentials::service_cookie_origin::cookie_origin_for_service;
use crate::state::data_dir::app_data_root::AppDataRoot;
use cookie_store_wrapper::cookie_store::CookieStore;
use chrono::Utc;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct EditWebCredentialResponse {
  pub credential: CredentialPayload,
}

/// Edit an existing web-login (cookie) credential, identified by its stable
/// id.
///
/// `name` replaces the label; an empty string clears it. `cookie_header`
/// replaces the cookies when provided (non-empty). Omitted fields are left
/// unchanged. The common case is simply renaming the credential.
#[tauri::command]
pub async fn edit_web_credential_command(
  app_data_root: State<'_, AppDataRoot>,
  credential_id: String,
  name: Option<String>,
  cookie_header: Option<String>,
) -> Result<EditWebCredentialResponse, String> {
  info!("edit_web_credential_command called for: {}", credential_id);

  let creds_dir = app_data_root.credentials_dir();

  let mut credential = creds_dir
      .find_credential_by_id(&credential_id)
      .map_err(|err| {
        error!("Error looking up credential {}: {}", credential_id, err);
        format!("Error looking up credential {}: {}", credential_id, err)
      })?
      .ok_or_else(|| {
        let message = format!("No credential found for id {}", credential_id);
        error!("{}", message);
        message
      })?;

  let CredentialSecret::Cookies(existing_cookie) = &credential.secret else {
    let message = format!("AuthCredential {} is not a cookie credential", credential_id);
    error!("{}", message);
    return Err(message);
  };
  let existing_cookies = existing_cookie.cookies.clone();
  let existing_failed_at = existing_cookie.failed_at;
  let existing_succeeded_at = existing_cookie.succeeded_at;
  let existing_grok_data = existing_cookie.grok_data.clone();

  if let Some(new_cookie_header) = cookie_header {
    let new_cookie_header = new_cookie_header.trim().to_string();
    if !new_cookie_header.is_empty() {
      let Some(cookie_origin) = cookie_origin_for_service(credential.service) else {
        let message = format!("Service {} has no cookie origin", credential.service);
        error!("{}", message);
        return Err(message);
      };
      let new_cookies = CookieStore::from_cookie_header(&new_cookie_header, &cookie_origin);
      if new_cookies != existing_cookies {
        credential.secret = CredentialSecret::Cookies(CookieCredential {
          updated_at: Some(Utc::now()),
          failed_at: existing_failed_at,
          succeeded_at: existing_succeeded_at,
          // New cookies are a new session; the old one's death no longer applies.
          relogin_required_since: None,
          // Hand-pasted cookies came from an unknown browser; drop the
          // recorded UA rather than claim one that may no longer match.
          user_agent: None,
          grok_data: existing_grok_data,
          cookies: new_cookies,
        });
      }
    }
  }

  if let Some(new_name) = name {
    let new_name = new_name.trim().to_string();
    credential.name = if new_name.is_empty() { None } else { Some(new_name) };
  }

  creds_dir
      .save_credential(&credential)
      .map_err(|err| {
        error!("Error saving credential {}: {}", credential_id, err);
        format!("Error saving credential {}: {}", credential_id, err)
      })?;

  Ok(EditWebCredentialResponse {
    credential: CredentialPayload::from_auth_credential(&credential),
  })
}
