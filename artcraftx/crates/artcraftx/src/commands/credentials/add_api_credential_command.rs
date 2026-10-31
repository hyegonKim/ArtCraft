use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::credentials::api_key_credential::ApiKeyCredential;
use crate::credentials::auth_credential::{AuthCredential, CredentialSecret};
use core_types::enums::generation_source::{CredentialKind, GenerationSource};
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct AddApiCredentialResponse {
  pub credential: CredentialPayload,
}

/// Create a new API-key credential file for a service.
///
/// Files follow the naming scheme `{service}.toml`, `{service}_2.toml`, ...
/// Multiple credentials per service are allowed.
#[tauri::command]
pub async fn add_api_credential_command(
  app_data_root: State<'_, AppDataRoot>,
  service: GenerationSource,
  api_key: String,
  name: Option<String>,
) -> Result<AddApiCredentialResponse, String> {
  info!("add_api_credential_command called for service: {}", service);

  if service.kind() != CredentialKind::ApiKey {
    let message = format!("Service {} is not an API key service", service);
    error!("{}", message);
    return Err(message);
  }

  let api_key = api_key.trim().to_string();
  if api_key.is_empty() {
    return Err("API key must not be empty".to_string());
  }

  let creds_dir = app_data_root.credentials_dir();
  let source_path = creds_dir.next_available_credential_path(service);

  let credential = AuthCredential {
    id: creds_dir.generate_unique_credential_id(),
    service,
    name: normalize_name(name),
    secret: CredentialSecret::ApiKey(ApiKeyCredential::new(api_key)),
    user_info: None,
    source_path,
  };

  creds_dir
      .save_credential(&credential)
      .map_err(|err| {
        error!("Error saving credential: {}", err);
        format!("Error saving credential: {}", err)
      })?;

  Ok(AddApiCredentialResponse {
    credential: CredentialPayload::from_auth_credential(&credential),
  })
}

fn normalize_name(name: Option<String>) -> Option<String> {
  name.map(|value| value.trim().to_string())
      .filter(|value| !value.is_empty())
}
