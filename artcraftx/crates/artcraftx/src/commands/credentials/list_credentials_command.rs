use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct ListCredentialsResponse {
  pub credentials: Vec<CredentialPayload>,
}

#[tauri::command]
pub async fn list_credentials_command(
  app_data_root: State<'_, AppDataRoot>,
) -> Result<ListCredentialsResponse, String> {
  info!("list_credentials_command called");

  let credentials = app_data_root
      .credentials_dir()
      .load_credentials()
      .map_err(|err| {
        error!("Error listing credentials: {}", err);
        format!("Error listing credentials: {}", err)
      })?;

  Ok(ListCredentialsResponse {
    credentials: credentials
        .iter()
        .map(CredentialPayload::from_auth_credential)
        .collect(),
  })
}
