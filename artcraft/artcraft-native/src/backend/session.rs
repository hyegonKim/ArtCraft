//! Where the signed-in session lives: our own file under `~/Artcraft/credentials`, seeded on
//! first run from the Tauri app's cookie jar so an existing ArtCraft login carries over.

use std::fs;
use std::path::{Path, PathBuf};

use artcraft_client::credentials::storyteller_avt_cookie::StorytellerAvtCookie;
use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::credentials::storyteller_session_cookie::StorytellerSessionCookie;
use artcraft_client::utils::api_host::ApiHost;
use log::{info, warn};
use serde::{Deserialize, Serialize};

/// The Tauri app's identifier: its cookie jar lives in `<cache dir>/<identifier>/.cookies`.
const TAURI_APP_IDENTIFIER: &str = "ai.artcraft.app";
const TAURI_COOKIES_FILENAME: &str = ".cookies";
const CREDENTIALS_FILENAME: &str = "native_session.json";

/// The two ArtCraft cookies, as sent on every request (`Cookie: visitor=…; session=…`).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
  pub session: String,
  #[serde(default)]
  pub visitor: Option<String>,
}

impl Credentials {
  pub fn to_set(&self) -> StorytellerCredentialSet {
    StorytellerCredentialSet::initialize(self.visitor.clone().map(StorytellerAvtCookie::new), Some(StorytellerSessionCookie::new(self.session.clone())))
  }
}

/// `~/Artcraft`, shared with the Tauri app (`AppDataRoot`).
pub fn data_root() -> PathBuf {
  directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from(".")).join("Artcraft")
}

/// The API host from `~/Artcraft/settings/env_configs.json` (production unless it says
/// `"storyteller_host": "localhost"`).
pub fn api_host(root: &Path) -> ApiHost {
  #[derive(Deserialize)]
  struct EnvConfigs {
    storyteller_host: Option<String>,
    storyteller_port: Option<u32>,
  }
  let path = root.join("settings").join("env_configs.json");
  let configs = fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<EnvConfigs>(&s).ok());
  match configs {
    Some(EnvConfigs { storyteller_host: Some(host), storyteller_port }) if host == "localhost" => ApiHost::Localhost { port: storyteller_port.unwrap_or(12345) },
    _ => ApiHost::Storyteller,
  }
}

/// The saved session, or the Tauri app's (imported and saved for next time).
pub fn load(root: &Path, allow_tauri_import: bool) -> Option<Credentials> {
  load_with_importer(root, allow_tauri_import, import_tauri_cookies)
}

fn load_with_importer(root: &Path, allow_tauri_import: bool, import: impl FnOnce() -> Option<Credentials>) -> Option<Credentials> {
  let path = credentials_path(root);
  if let Ok(text) = fs::read_to_string(&path) {
    match serde_json::from_str::<Credentials>(&text) {
      Ok(creds) if !creds.session.is_empty() => return Some(creds),
      Ok(_) => {},
      Err(err) => warn!("Ignoring unreadable {}: {err}", path.display()),
    }
  }
  if !allow_tauri_import {
    return None;
  }
  let imported = import()?;
  info!("Imported the ArtCraft login from the desktop app's cookie jar");
  save(root, &imported);
  Some(imported)
}

pub fn save(root: &Path, creds: &Credentials) {
  let path = credentials_path(root);
  let result = path.parent().map_or(Ok(()), fs::create_dir_all).and_then(|_| fs::write(&path, serde_json::to_vec_pretty(creds).unwrap_or_default()));
  if let Err(err) = result {
    warn!("Couldn't save the session to {}: {err}", path.display());
  }
}

pub fn delete(root: &Path) {
  let path = credentials_path(root);
  if let Err(err) = fs::remove_file(&path) {
    if err.kind() != std::io::ErrorKind::NotFound {
      warn!("Couldn't delete {}: {err}", path.display());
    }
  }
}

fn credentials_path(root: &Path) -> PathBuf {
  root.join("credentials").join(CREDENTIALS_FILENAME)
}

/// Reads `session` and `visitor` for the ArtCraft API from the Tauri HTTP plugin's jar: a JSON
/// array of `cookie_store` cookies (`{"raw_cookie": "...", "domain": {...}, "expires": {...}}`).
fn import_tauri_cookies() -> Option<Credentials> {
  let path = directories::BaseDirs::new()?.cache_dir().join(TAURI_APP_IDENTIFIER).join(TAURI_COOKIES_FILENAME);
  let text = fs::read_to_string(&path).ok()?;
  let cookies: Vec<serde_json::Value> = serde_json::from_str(&text).ok()?;
  let now = chrono::Utc::now();
  let mut creds = Credentials::default();
  for entry in &cookies {
    let for_api = entry.get("domain").map(|d| d.to_string()).is_some_and(|d| d.contains("storyteller.ai"));
    let expired = entry.pointer("/expires/AtUtc").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).is_some_and(|t| t < now);
    let Some(raw) = entry.get("raw_cookie").and_then(|v| v.as_str()) else {
      continue;
    };
    if !for_api || expired {
      continue;
    }
    let Ok(cookie) = cookie::Cookie::parse(raw) else {
      continue;
    };
    match cookie.name() {
      "session" => creds.session = cookie.value().to_owned(),
      "visitor" => creds.visitor = Some(cookie.value().to_owned()),
      _ => {},
    }
  }
  (!creds.session.is_empty()).then_some(creds)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::backend::providers::accounts::ProviderAccounts;
  use crate::backend::providers::jobs::{JobStore, ProviderJob};
  use crate::backend::providers::Provider;
  use crate::launch::LaunchOptions;

  #[test]
  fn isolated_missing_session_never_imports_legacy_cookies() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = load_with_importer(dir.path(), false, || panic!("must not access global cookie jar"));
    assert!(loaded.is_none());
    assert!(!credentials_path(dir.path()).exists());
    assert!(load(dir.path(), false).is_none());
  }

  #[test]
  fn isolated_invalid_session_does_not_fall_back_or_rewrite_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = credentials_path(dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "invalid fixture session").unwrap();
    assert!(load_with_importer(dir.path(), false, || panic!("must not import after a parse failure")).is_none());
    assert_eq!(fs::read_to_string(path).unwrap(), "invalid fixture session");
  }

  #[test]
  fn existing_session_has_precedence_without_importing() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &Credentials { session: "existing-fixture".into(), visitor: Some("fixture-visitor".into()) });
    for allow_import in [false, true] {
      let loaded = load_with_importer(dir.path(), allow_import, || panic!("saved session must win")).unwrap();
      assert_eq!(loaded.session, "existing-fixture");
      assert_eq!(loaded.visitor.as_deref(), Some("fixture-visitor"));
    }
  }

  #[test]
  fn default_mode_still_imports_and_saves_a_legacy_session() {
    let dir = tempfile::tempdir().unwrap();
    let loaded = load_with_importer(dir.path(), true, || Some(Credentials { session: "imported-fixture".into(), visitor: None })).unwrap();
    assert_eq!(loaded.session, "imported-fixture");
    assert!(credentials_path(dir.path()).is_file());
    assert_eq!(load(dir.path(), false).unwrap().session, "imported-fixture");
  }

  #[test]
  fn explicit_root_owns_session_providers_jobs_and_ui_storage() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("isolated");
    let unrelated = dir.path().join("unrelated");
    fs::create_dir_all(&unrelated).unwrap();
    fs::write(unrelated.join("sentinel"), "unchanged").unwrap();
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(root.join("settings/env_configs.json"), r#"{"storyteller_host":"production.invalid"}"#).unwrap();
    let launch = LaunchOptions::parse([
      "--data-dir".into(), root.clone().into_os_string(),
      "--api-base-url".into(), "http://localhost:4203".into(),
    ]).unwrap();
    let settings = launch.resolve(|| panic!("must not read global root or settings"));
    assert_eq!(settings.api_host.to_api_hostname_and_scheme(), "http://localhost:4203");
    assert!(!settings.import_tauri_cookies);
    assert!(load(&settings.data_root, settings.import_tauri_cookies).is_none());

    save(&settings.data_root, &Credentials { session: "isolated-fixture".into(), visitor: None });
    assert_eq!(load(&settings.data_root, false).unwrap().session, "isolated-fixture");
    let accounts = ProviderAccounts::load(&settings.data_root);
    assert_eq!(accounts.midjourney_account(), None);
    accounts.set_fal_key("isolated-fixture-key").unwrap();
    assert_eq!(ProviderAccounts::load(&root).fal_key().as_deref(), Some("isolated-fixture-key"));
    let job = ProviderJob {
      id: "local_isolated".into(), provider: Provider::Fal, model_id: "fixture-model".into(), prompt: "fixture".into(),
      created_at: 1, batch_count: 1, provider_job_id: "fixture-job".into(), maybe_status_url: None, maybe_response_url: None,
      maybe_prompt_token: None, batch_token: "fixture-batch".into(), upload_tokens: vec![], uploaded: vec![],
    };
    JobStore::load(&settings.data_root).put(job.clone());
    assert_eq!(JobStore::load(&root).all(), vec![job]);
    assert!(root.join("credentials/native_session.json").is_file());
    assert!(root.join("credentials/fal.api_key.txt").is_file());
    assert!(root.join("state/native_provider_jobs.json").is_file());
    assert_eq!(launch.native_options(egui::ViewportBuilder::default()).persistence_path, Some(root.join("ui/app.ron")));
    accounts.clear_fal_key().unwrap();
    JobStore::load(&root).remove("local_isolated");
    delete(&root);
    assert!(JobStore::load(&root).all().is_empty());
    assert!(!credentials_path(&root).exists());
    assert_eq!(fs::read_to_string(unrelated.join("sentinel")).unwrap(), "unchanged");
    assert_eq!(fs::read_dir(&unrelated).unwrap().count(), 1);
  }
}
