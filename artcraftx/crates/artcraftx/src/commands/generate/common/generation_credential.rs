use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;

use crate::commands::generate::generate_error::{CredentialProblemReason, GenerateError};
use crate::credentials::auth_credential::AuthCredential;
use crate::state::data_dir::app_data_root::AppDataRoot;

/// Resolve the stored credential a generation request names via
/// `credential_id`. Every failure is a [`GenerateError::CredentialProblem`],
/// which flashes the frontend's dismissable credential-error modal.
pub fn resolve_generation_credential(
  maybe_credential_id: Option<&str>,
  app_data_root: &AppDataRoot,
) -> Result<AuthCredential, GenerateError> {
  let credential_id = maybe_credential_id
      .filter(|id| !id.trim().is_empty())
      .ok_or(GenerateError::CredentialProblem(
        CredentialProblemReason::NoCredentialSupplied,
      ))?;

  let maybe_credential = app_data_root
      .credentials_dir()
      .find_credential_by_id(credential_id)
      .map_err(GenerateError::from)?;

  maybe_credential.ok_or_else(|| {
    GenerateError::CredentialProblem(CredentialProblemReason::CredentialNotFound {
      credential_id: credential_id.to_string(),
    })
  })
}

/// Rebuild the web-session credential set from a stored cookie credential.
pub fn storyteller_creds_from_credential(
  credential: &AuthCredential,
) -> Result<StorytellerCredentialSet, GenerateError> {
  let cookie = credential.cookies().ok_or_else(|| {
    credential_not_usable(credential, "the account has no session cookies")
  })?;

  StorytellerCredentialSet::parse_multi_cookie_header(&cookie.cookie_header())
      .map_err(|err| {
        credential_not_usable(
          credential,
          &format!("the stored session cookies could not be parsed ({})", err),
        )
      })?
      .filter(|creds| !creds.is_empty())
      .ok_or_else(|| {
        credential_not_usable(credential, "the stored session cookies are empty")
      })
}

pub fn credential_not_usable(credential: &AuthCredential, reason: &str) -> GenerateError {
  GenerateError::CredentialProblem(CredentialProblemReason::CredentialNotUsable {
    credential_id: credential.id.to_string(),
    reason: reason.to_string(),
  })
}
