//! Where generations made on other services get backed up.
//!
//! The user opts in under Settings → Account Backup and picks one of their
//! ArtCraft accounts. Every third-party completion path (Higgsfield,
//! Midjourney, Grok, Fal, and whatever comes next) asks [`resolve_backup_target`]
//! at delivery time and, when it answers, uploads the results (and a prompt
//! record) to that account. Nothing is cached: the preferences and the
//! credential file are re-read per task so changes apply immediately.

use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::utils::api_host::ApiHost;
use core_types::enums::generation_source::GenerationSource;
use core_types::identifiers::credential_id::CredentialId;
use log::{info, warn};

use crate::credentials::auth_credential::AuthCredential;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::services::artcraft_api_host::maybe_artcraft_api_host_for_service;

/// The ArtCraft account third-party generations are backed up to.
#[derive(Clone)]
pub struct BackupTarget {
  pub credential_id: CredentialId,
  pub api_host: ApiHost,
  pub creds: StorytellerCredentialSet,
}

/// Whether a stored credential can receive backups: an ArtCraft account with
/// a web session (uploads and prompt records need the session cookies).
pub fn is_backup_eligible(credential: &AuthCredential) -> bool {
  matches!(
    credential.service,
    GenerationSource::Artcraft | GenerationSource::ArtcraftLocal | GenerationSource::ArtcraftCookies
  ) && credential.cookies().is_some()
}

/// The configured backup account, if backups are on and the chosen
/// credential still exists and is usable. Logs why when it doesn't.
pub fn resolve_backup_target(
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
) -> Option<BackupTarget> {
  let backup = match app_preferences.get() {
    Ok(prefs) => prefs.backup,
    Err(err) => {
      warn!("[Backup] Could not read app preferences; not backing up: {}", err);
      return None;
    }
  };
  if !backup.enabled {
    return None;
  }
  let Some(credential_id) = backup.maybe_artcraft_credential_id.filter(|id| !id.trim().is_empty()) else {
    info!("[Backup] Backups are on but no ArtCraft account is selected; not backing up");
    return None;
  };

  let credential = match app_data_root.credentials_dir().find_credential_by_id(&credential_id) {
    Ok(Some(credential)) => credential,
    Ok(None) => {
      warn!("[Backup] Backup account {} no longer exists; not backing up", credential_id);
      return None;
    }
    Err(err) => {
      warn!("[Backup] Could not load backup account {}: {}", credential_id, err);
      return None;
    }
  };

  backup_target_for_credential(&credential)
}

/// The backup target a specific credential represents, if it is eligible.
pub fn backup_target_for_credential(credential: &AuthCredential) -> Option<BackupTarget> {
  if !is_backup_eligible(credential) {
    warn!("[Backup] Credential {} ({}) is not an ArtCraft web-session account; not backing up", credential.id, credential.service);
    return None;
  }
  let api_host = maybe_artcraft_api_host_for_service(credential.service)?;
  let cookie = credential.cookies()?;
  let creds = match StorytellerCredentialSet::parse_multi_cookie_header(&cookie.cookie_header()) {
    Ok(Some(creds)) if !creds.is_empty() => creds,
    Ok(_) => {
      warn!("[Backup] Backup account {} has no usable session cookies; not backing up", credential.id);
      return None;
    }
    Err(err) => {
      warn!("[Backup] Backup account {} has unparsable session cookies ({}); not backing up", credential.id, err);
      return None;
    }
  };
  Some(BackupTarget { credential_id: credential.id.clone(), api_host, creds })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::auth_credential::CredentialSecret;
  use crate::credentials::api_key_credential::ApiKeyCredential;
  use crate::credentials::cookie_credential::CookieCredential;
  use cookie_store_wrapper::cookie_store::CookieStore;
  use reqwest::Url;
  use std::path::PathBuf;

  fn cookie_credential(service: GenerationSource, header: &str) -> AuthCredential {
    let origin = Url::parse("https://artcraft.ai/").unwrap();
    AuthCredential {
      id: CredentialId::generate(),
      service,
      name: None,
      secret: CredentialSecret::Cookies(CookieCredential::new(CookieStore::from_cookie_header(header, &origin))),
      user_info: None,
      source_path: PathBuf::from("/tmp/never_written.toml"),
    }
  }

  #[test]
  fn only_artcraft_web_sessions_are_eligible() {
    assert!(is_backup_eligible(&cookie_credential(GenerationSource::Artcraft, "session=abc")));
    assert!(is_backup_eligible(&cookie_credential(GenerationSource::ArtcraftCookies, "session=abc")));
    assert!(!is_backup_eligible(&cookie_credential(GenerationSource::HiggsfieldCookies, "__client=abc")));
    let api_key = AuthCredential {
      secret: CredentialSecret::ApiKey(ApiKeyCredential::new("sk-123")),
      service: GenerationSource::ArtcraftApi,
      ..cookie_credential(GenerationSource::Artcraft, "session=abc")
    };
    assert!(!is_backup_eligible(&api_key), "API keys can't drive web-session uploads");
  }

  #[test]
  fn target_carries_the_session_and_host() {
    let target = backup_target_for_credential(&cookie_credential(GenerationSource::Artcraft, "session=abc")).expect("eligible");
    assert!(!target.creds.is_empty());
    assert!(matches!(target.api_host, ApiHost::Storyteller));
    assert!(backup_target_for_credential(&cookie_credential(GenerationSource::HiggsfieldCookies, "__client=abc")).is_none());
  }
}
