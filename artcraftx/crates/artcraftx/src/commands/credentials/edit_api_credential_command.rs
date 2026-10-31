use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::credentials::api_key_credential::ApiKeyCredential;
use crate::credentials::auth_credential::CredentialSecret;
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct EditApiCredentialResponse {
  pub credential: CredentialPayload,
}

/// Edit an existing API-key credential, identified by its stable id.
///
/// `api_key` replaces the key when provided (non-empty). `name` replaces the
/// label; an empty string clears it. Omitted fields are left unchanged.
#[tauri::command]
pub async fn edit_api_credential_command(
  app_data_root: State<'_, AppDataRoot>,
  credential_id: String,
  api_key: Option<String>,
  name: Option<String>,
) -> Result<EditApiCredentialResponse, String> {
  info!("edit_api_credential_command called for: {}", credential_id);

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

  let CredentialSecret::ApiKey(existing_key) = &credential.secret else {
    let message = format!("AuthCredential {} is not an API key credential", credential_id);
    error!("{}", message);
    return Err(message);
  };

  if let Some(new_api_key) = api_key {
    let new_api_key = new_api_key.trim().to_string();
    if !new_api_key.is_empty() && new_api_key != existing_key.api_key {
      credential.secret = CredentialSecret::ApiKey(ApiKeyCredential::new(new_api_key));
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

  Ok(EditApiCredentialResponse {
    credential: CredentialPayload::from_auth_credential(&credential),
  })
}
