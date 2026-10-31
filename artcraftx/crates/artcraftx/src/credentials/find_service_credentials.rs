use log::warn;

use crate::credentials::auth_credential::AuthCredential;
use core_types::enums::generation_source::GenerationSource;
use crate::state::data_dir::app_data_root::AppDataRoot;

/// Find the first stored credential for a service, if any.
///
/// Background threads use this to authenticate without a request-scoped
/// credential id (e.g. the FAL polling thread picking up the user's FAL
/// API key).
pub fn find_first_credential_for_service(
  app_data_root: &AppDataRoot,
  service: GenerationSource,
) -> Option<AuthCredential> {
  let credentials = match app_data_root.credentials_dir().load_credentials() {
    Ok(credentials) => credentials,
    Err(err) => {
      warn!("Could not load credentials for service {}: {}", service, err);
      return None;
    }
  };

  credentials.into_iter().find(|c| c.service == service)
}

/// The first stored FAL API key, if any.
pub fn find_fal_api_key(app_data_root: &AppDataRoot) -> Option<String> {
  find_first_credential_for_service(app_data_root, GenerationSource::FalApi)
      .and_then(|credential| credential.api_key().map(|key| key.api_key.clone()))
}
