use crate::credentials::api_key_credential::ApiKeyCredential;
use crate::credentials::cookie_credential::CookieCredential;
use crate::credentials::credential_toml::CredentialToml;
use core_types::enums::generation_source::{CredentialKind, GenerationSource};
use crate::credentials::credential_user_info::CredentialUserInfo;
use crate::error::artcraftx_credential_error::ArtcraftXCredentialError;
use chrono::Utc;
use std::path::{Path, PathBuf};
use core_types::identifiers::credential_id::CredentialId;

/// A validated, in-app credential.
///
/// Unlike the on-disk [`CredentialToml`], an `AuthCredential` always carries
/// exactly one secret and remembers which file it was loaded from, so the
/// app can rewrite that file in place (refreshed cookies, success/failure
/// timestamps, etc.)
#[derive(Clone, Debug)]
pub struct AuthCredential {
  /// The credential's stable identity (`credential_{entropy}`), stored in the
  /// TOML file. Hidden from users, but the effective primary identifier —
  /// file names can be freely renamed.
  pub id: CredentialId,
  pub service: GenerationSource,
  /// Optional user-facing label to tell multiple credentials for the same
  /// service apart. Empty by default.
  pub name: Option<String>,
  pub secret: CredentialSecret,
  pub user_info: Option<CredentialUserInfo>,
  /// The TOML file this credential was loaded from (and saves back to).
  pub source_path: PathBuf,
}

/// The secret payload of a credential. Mutually exclusive by construction.
#[derive(Clone, Debug)]
pub enum CredentialSecret {
  Cookies(CookieCredential),
  ApiKey(ApiKeyCredential),
}

impl CredentialSecret {
  pub fn kind(&self) -> CredentialKind {
    match self {
      Self::Cookies(_) => CredentialKind::Cookies,
      Self::ApiKey(_) => CredentialKind::ApiKey,
    }
  }
}

impl AuthCredential {
  /// Load a credential, minting (and persisting) an id if the file lacks
  /// one so the identity stays stable across loads.
  pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ArtcraftXCredentialError> {
    let path = path.as_ref();
    let file = CredentialToml::load(path)?;
    let needs_id_backfill = file.id.is_none();
    let credential = file.into_auth_credential(path.to_path_buf())?;
    if needs_id_backfill {
      if let Err(err) = credential.save() {
        log::warn!("Could not persist backfilled credential id for {:?}: {}", path, err);
      }
    }
    Ok(credential)
  }

  /// Rewrite this credential's TOML file in place.
  pub fn save(&self) -> Result<(), ArtcraftXCredentialError> {
    CredentialToml::from_auth_credential(self).save(&self.source_path)
  }

  pub fn kind(&self) -> CredentialKind {
    self.secret.kind()
  }

  /// Whether the stored session is known-dead (see
  /// [`CookieCredential::relogin_required_since`]). API-key credentials
  /// never need a browser re-login.
  pub fn needs_relogin(&self) -> bool {
    self.cookies().is_some_and(CookieCredential::needs_relogin)
  }

  /// Record that the provider rejected this credential's session and
  /// persist it, so every later caller skips the API until the user logs in
  /// again. Returns `Ok(true)` when the credential was newly marked (the
  /// moment to tell the user), `Ok(false)` if it was already marked or isn't
  /// a cookie credential.
  pub fn mark_relogin_required(&mut self) -> Result<bool, ArtcraftXCredentialError> {
    let CredentialSecret::Cookies(cookie) = &mut self.secret else {
      return Ok(false);
    };
    let newly_marked = cookie.mark_relogin_required(Utc::now());
    self.save()?;
    Ok(newly_marked)
  }

  /// The credential's file name within the credentials directory
  /// (e.g. `fal_api_2.toml`). Informational — the stable identifier is
  /// [`AuthCredential::id`]; users can rename files freely.
  pub fn file_name(&self) -> String {
    self.source_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
  }

  pub fn cookies(&self) -> Option<&CookieCredential> {
    match &self.secret {
      CredentialSecret::Cookies(cookie) => Some(cookie),
      CredentialSecret::ApiKey(_) => None,
    }
  }

  pub fn api_key(&self) -> Option<&ApiKeyCredential> {
    match &self.secret {
      CredentialSecret::ApiKey(api_key) => Some(api_key),
      CredentialSecret::Cookies(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // A cookie credential carrying Grok statsig data must round-trip: the
  // `grok_data` table has to serialize before the `cookies` array-of-tables, or
  // the TOML is invalid.
  #[test]
  fn cookie_credential_with_grok_data_round_trips() {
    use crate::credentials::cookie_credential::CookieCredential;
    use crate::credentials::cookie_credential_grok_extra_pieces::{CookieCredentialGrokExtraPieces, GrokStatsigCapture};
    use chrono::Utc;
    use cookie_store_wrapper::cookie_store::CookieStore;
    use grok_consumer_statsig::StatsigMaterial;
    use reqwest::Url;

    let material = StatsigMaterial::from_seed_b64("7LKU1SbbPgGmlkhdNLb4ltpqrlenNVC2KnvBRD5ly/VV9ueQboie/eZABqiclvh1");
    let capture = GrokStatsigCapture {
      material: material.clone(),
      statsig_id: "XbHvyYh7hmNc+8sVAGnrpcuHN/MK+mgN63cmnBljOJaoCKu6zTPVw6C7HVv1wculKCNmYVum+L1h0IjFmtyZD/C53M+ZXg".to_string(),
      method: "POST".to_string(),
      path: "/rest/app-chat/conversations/new".to_string(),
      captured_at: Utc::now(),
    };

    let mut cookies = CookieStore::empty();
    cookies.insert_named("sso", "token-value", &Url::parse("https://grok.com/").unwrap());

    let now = Utc::now();
    let mut cookie = CookieCredential::new(cookies);
    cookie.updated_at = Some(now);
    cookie.grok_data = Some(CookieCredentialGrokExtraPieces::fresh(capture.clone(), now));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grok_cookies.toml");
    let credential = AuthCredential {
      id: CredentialId::generate(),
      service: GenerationSource::GrokCookies,
      name: None,
      secret: CredentialSecret::Cookies(cookie),
      user_info: None,
      source_path: path.clone(),
    };
    credential.save().unwrap();

    let loaded = AuthCredential::load_from_file(&path).unwrap();
    let loaded_grok = loaded.cookies().unwrap().grok_data.as_ref().unwrap();
    assert_eq!(loaded_grok.statsig_material.as_ref(), Some(&material));
    assert!(loaded_grok.statsig_fetched_at.is_some());
    assert!(loaded_grok.statsig_refresh_at.is_some());
    let latest = loaded_grok.latest_statsig.as_ref().expect("latest statsig persisted");
    assert_eq!(latest.statsig_id, capture.statsig_id);
    assert_eq!(latest.method, "POST");
    assert_eq!(latest.path, "/rest/app-chat/conversations/new");
    assert_eq!(loaded_grok.latest_statsig_if_fresh(now), Some(capture.statsig_id.as_str()));
    assert!(loaded.cookies().unwrap().cookies.has_cookie("sso"));
  }

  #[test]
  fn save_and_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fal_api_key.toml");

    let credential = AuthCredential {
      id: CredentialId::generate(),
      service: GenerationSource::FalApi,
      name: Some("work account".to_string()),
      secret: CredentialSecret::ApiKey(ApiKeyCredential::new("fal-secret-key-12345")),
      user_info: None,
      source_path: path.clone(),
    };
    credential.save().unwrap();

    let loaded = AuthCredential::load_from_file(&path).unwrap();
    assert_eq!(loaded.id, credential.id);
    assert_eq!(loaded.service, GenerationSource::FalApi);
    assert_eq!(loaded.name.as_deref(), Some("work account"));
    assert_eq!(loaded.api_key().unwrap().api_key, "fal-secret-key-12345");
    assert_eq!(loaded.api_key().unwrap().printable_partial_prefix, "fal-s");
    assert_eq!(loaded.source_path, path);
    assert_eq!(loaded.file_name(), "fal_api_key.toml");
  }
}
