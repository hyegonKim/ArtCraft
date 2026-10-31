use crate::commands::credentials::credential_payload::CredentialPayload;
use crate::credentials::cookie_credential::CookieCredential;
use crate::credentials::auth_credential::{AuthCredential, CredentialSecret};
use core_types::enums::generation_source::{CredentialKind, GenerationSource};
use crate::credentials::credential_user_info::CredentialUserInfo;
use crate::credentials::service_cookie_origin::cookie_origin_for_service;
use crate::error::artcraftx_error::ArtcraftXError;
use cookie_store_wrapper::cookie_store::CookieStore;
use crate::credentials::cookie_credential_grok_extra_pieces::{CookieCredentialGrokExtraPieces, GrokStatsigCapture};
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::data_dir::subdirectory::app_credentials_dir::AppCredentialsDir;
use chrono::Utc;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

/// The captured result of a web login, ready to persist.
pub struct WebCredentialSave {
  pub service: GenerationSource,
  pub cookies: CookieStore,
  pub maybe_user_info: Option<CredentialUserInfo>,
  /// Preemptively captured statsig (Grok only; `None` for other sites).
  pub maybe_statsig: Option<GrokStatsigCapture>,
  /// The User-Agent the capturing webview presented, when the site pins one.
  /// Bot-protection cookies are bound to it; see `CookieCredential::user_agent`.
  pub maybe_user_agent: Option<String>,
  /// An existing credential to refresh in place (a re-login). When it still
  /// exists, exactly that file is rewritten; when it's gone, a new one is
  /// created. `None` upserts the service's managed credential instead.
  pub maybe_target_credential_id: Option<String>,
}

/// Save a web-login (cookie) credential to the credentials directory.
///
/// With a target credential id (a re-login), that exact credential is
/// rewritten if it still exists, keeping its id, label, and file. Otherwise
/// this upserts by service: if a managed cookie credential for `service`
/// already exists, its file (and label) are reused and the cookies refreshed;
/// else a new `{service}.toml` file is created. This keeps the login flow
/// from piling up a new file on every re-login while still letting users
/// hand-maintain multiple accounts.
///
/// Fresh cookies are a fresh session, so any "needs re-login" mark is cleared.
pub fn save_web_credential(
  creds_dir: &AppCredentialsDir,
  save: WebCredentialSave,
) -> Result<AuthCredential, ArtcraftXError> {
  let all_credentials = creds_dir.load_credentials()?;

  let targeted = save.maybe_target_credential_id.as_deref().and_then(|target_id| {
    let found = all_credentials.iter().find(|credential| {
      credential.id.as_str() == target_id
          && credential.service == save.service
          && credential.kind() == CredentialKind::Cookies
    });
    if found.is_none() {
      info!("Re-login target credential {} no longer exists; saving a new credential instead", target_id);
    }
    found.cloned()
  });

  let existing = targeted.or_else(|| {
    all_credentials.into_iter().find(|credential| {
      credential.service == save.service && credential.kind() == CredentialKind::Cookies
    })
  });

  let now = Utc::now();

  // A fresh capture stamps fetched/expiry times; without one, preserve whatever
  // the existing credential already had.
  let grok_data = match save.maybe_statsig {
    Some(capture) => Some(CookieCredentialGrokExtraPieces::fresh(capture, now)),
    None => existing
        .as_ref()
        .and_then(|credential| credential.cookies())
        .and_then(|cookie| cookie.grok_data.clone()),
  };

  let cookie = CookieCredential {
    updated_at: Some(now),
    failed_at: None,
    succeeded_at: None,
    relogin_required_since: None,
    user_agent: save.maybe_user_agent,
    grok_data,
    cookies: save.cookies,
  };

  let credential = match existing {
    Some(existing) => AuthCredential {
      id: existing.id,
      service: existing.service,
      name: existing.name,
      secret: CredentialSecret::Cookies(cookie),
      user_info: merge_user_info(existing.user_info, save.maybe_user_info),
      source_path: existing.source_path,
    },
    None => AuthCredential {
      id: creds_dir.generate_unique_credential_id(),
      service: save.service,
      name: None,
      secret: CredentialSecret::Cookies(cookie),
      user_info: save.maybe_user_info,
      source_path: creds_dir.next_available_credential_path(save.service),
    },
  };

  creds_dir.save_credential(&credential)?;
  Ok(credential)
}

/// Prefer freshly detected identity fields, falling back to whatever the
/// existing file already recorded, so a login that can't read (say) an email
/// doesn't wipe one the user previously had.
fn merge_user_info(
  existing: Option<CredentialUserInfo>,
  fresh: Option<CredentialUserInfo>,
) -> Option<CredentialUserInfo> {
  match (existing, fresh) {
    (None, fresh) => fresh,
    (existing, None) => existing,
    (Some(existing), Some(fresh)) => Some(CredentialUserInfo {
      username: fresh.username.or(existing.username),
      email: fresh.email.or(existing.email),
    }),
  }
}

#[derive(Serialize)]
pub struct AddWebCredentialResponse {
  pub credential: CredentialPayload,
}

/// Manually add a web-login (cookie) credential file for a service, given a
/// full `Cookie:` header. The automated login flow saves via
/// [`save_web_credential`]; this command is the hand-entry path.
#[tauri::command]
pub async fn add_web_credential_command(
  app_data_root: State<'_, AppDataRoot>,
  service: GenerationSource,
  cookie_header: String,
  name: Option<String>,
) -> Result<AddWebCredentialResponse, String> {
  info!("add_web_credential_command called for service: {}", service);

  if service.kind() != CredentialKind::Cookies {
    let message = format!("Service {} is not a cookie service", service);
    error!("{}", message);
    return Err(message);
  }

  let cookie_header = cookie_header.trim().to_string();
  if cookie_header.is_empty() {
    return Err("Cookie header must not be empty".to_string());
  }

  let Some(cookie_origin) = cookie_origin_for_service(service) else {
    let message = format!("Service {} has no cookie origin", service);
    error!("{}", message);
    return Err(message);
  };
  let cookies = CookieStore::from_cookie_header(&cookie_header, &cookie_origin);
  if cookies.is_empty() {
    return Err("Cookie header contained no name=value cookies".to_string());
  }

  let credential = save_web_credential(
    app_data_root.credentials_dir(),
    WebCredentialSave {
      service,
      cookies,
      maybe_user_info: None,
      maybe_statsig: None,
      // Hand-entered cookies came from an unknown browser; record no UA.
      maybe_user_agent: None,
      maybe_target_credential_id: None,
    },
  ).map_err(|err| {
    error!("Error saving web credential: {}", err);
    format!("Error saving web credential: {}", err)
  })?;

  let credential = apply_optional_name(app_data_root.credentials_dir(), credential, name)?;

  Ok(AddWebCredentialResponse {
    credential: CredentialPayload::from_auth_credential(&credential),
  })
}

/// Apply an optional user-supplied label to a just-saved credential.
fn apply_optional_name(
  creds_dir: &AppCredentialsDir,
  mut credential: AuthCredential,
  name: Option<String>,
) -> Result<AuthCredential, String> {
  let Some(name) = name else {
    return Ok(credential);
  };
  let name = name.trim().to_string();
  credential.name = if name.is_empty() { None } else { Some(name) };
  creds_dir.save_credential(&credential).map_err(|err| {
    error!("Error saving credential label: {}", err);
    format!("Error saving credential label: {}", err)
  })?;
  Ok(credential)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::auth_credential::AuthCredential;
  use reqwest::Url;

  const SERVICE: GenerationSource = GenerationSource::HiggsfieldCookies;

  #[test]
  fn targeted_relogin_rewrites_that_credential_and_clears_the_mark() {
    let (_dir, root) = temp_root();
    let creds_dir = root.credentials_dir();
    let first = save(creds_dir, "__client=first", None);
    let second = save(creds_dir, "__client=second", None);
    // NB: without a target, a second login upserts the first managed file.
    assert_eq!(second.id, first.id);

    let mut expired = creds_dir.find_credential_by_id(first.id.as_str()).unwrap().unwrap();
    expired.name = Some("work".to_string());
    creds_dir.save_credential(&expired).unwrap();
    assert!(expired.mark_relogin_required().unwrap());
    assert!(creds_dir.find_credential_by_id(first.id.as_str()).unwrap().unwrap().needs_relogin());

    let refreshed = save(creds_dir, "__client=fresh", Some(first.id.as_str()));
    assert_eq!(refreshed.id, first.id, "same identity");
    assert_eq!(refreshed.source_path, first.source_path, "same file");
    assert_eq!(refreshed.name.as_deref(), Some("work"), "label kept");
    assert!(!refreshed.needs_relogin(), "fresh session clears the mark");
    assert!(refreshed.cookies().unwrap().cookies.has_cookie("__client"));
    assert_eq!(creds_dir.load_credentials().unwrap().len(), 1, "no second file");
  }

  #[test]
  fn targeted_relogin_creates_a_new_credential_when_the_target_is_gone() {
    let (_dir, root) = temp_root();
    let creds_dir = root.credentials_dir();
    let saved = save(creds_dir, "__client=fresh", Some("credential_deleted_meanwhile"));
    assert_ne!(saved.id.as_str(), "credential_deleted_meanwhile");
    assert_eq!(creds_dir.load_credentials().unwrap().len(), 1);
    assert!(!saved.needs_relogin());
  }

  // ── Helpers ──

  fn temp_root() -> (tempfile::TempDir, AppDataRoot) {
    let dir = tempfile::tempdir().unwrap();
    let root = AppDataRoot::create_existing(dir.path()).unwrap();
    (dir, root)
  }

  fn save(creds_dir: &AppCredentialsDir, cookie_header: &str, target: Option<&str>) -> AuthCredential {
    let origin = Url::parse("https://higgsfield.ai/").unwrap();
    save_web_credential(creds_dir, WebCredentialSave {
      service: SERVICE,
      cookies: CookieStore::from_cookie_header(cookie_header, &origin),
      maybe_user_info: None,
      maybe_statsig: None,
      maybe_user_agent: None,
      maybe_target_credential_id: target.map(str::to_string),
    }).unwrap()
  }
}
