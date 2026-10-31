use crate::credentials::api_key_credential::ApiKeyCredential;
use crate::credentials::cookie_credential::CookieCredential;
use crate::credentials::auth_credential::{AuthCredential, CredentialSecret};
use core_types::enums::generation_source::GenerationSource;
use crate::credentials::credential_user_info::CredentialUserInfo;
use crate::error::artcraftx_credential_error::ArtcraftXCredentialError;
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use core_types::identifiers::credential_id::CredentialId;

/// The on-disk (TOML) shape of a credential file.
///
/// This is the tolerant serialization layer: both secrets are optional so
/// that hand-written files always parse, and validation happens when
/// converting into the in-app [`AuthCredential`] type.
///
/// Example file (`~/Artcraft/artcraftx/credentials/higgsfield.toml`):
///
/// ```toml
/// id = "credential_01j9dq3v5x8k2m7n4p6r9t0wxy"
/// service = "higgsfield_cookies"
///
/// [user_info]
/// username = "creator123"
///
/// [[cookie.cookies]]
/// name = "session"
/// value = "abc123"
/// domain = "higgsfield.ai"
///
/// [[cookie.cookies]]
/// name = "other"
/// value = "value"
/// domain = "higgsfield.ai"
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CredentialToml {
  /// The credential's stable identity. Absent in older or hand-written
  /// files; loading backfills a freshly generated id.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub id: Option<CredentialId>,

  pub service: GenerationSource,

  /// Optional user-facing label. Empty by default.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cookie: Option<CookieCredential>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub api_key: Option<ApiKeyCredential>,

  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user_info: Option<CredentialUserInfo>,
}

impl CredentialToml {
  pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ArtcraftXCredentialError> {
    let path = path.as_ref();
    let contents = std::fs::read_to_string(path)
        .map_err(|source| ArtcraftXCredentialError::FileReadError {
          path: path.to_path_buf(),
          source,
        })?;
    toml::from_str(&contents)
        .map_err(|source| ArtcraftXCredentialError::TomlParseError {
          path: path.to_path_buf(),
          source,
        })
  }

  pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ArtcraftXCredentialError> {
    let path = path.as_ref();
    let contents = toml::to_string_pretty(self)
        .map_err(|source| ArtcraftXCredentialError::TomlSerializeError { source })?;
    std::fs::write(path, contents)
        .map_err(|source| ArtcraftXCredentialError::FileWriteError {
          path: path.to_path_buf(),
          source,
        })
  }

  /// Validate and convert into the in-app credential type.
  ///
  /// Exactly one of `cookie` / `api_key` must be present, and it must match
  /// the auth mechanism of `service`.
  pub fn into_auth_credential(self, source_path: PathBuf) -> Result<AuthCredential, ArtcraftXCredentialError> {
    let secret = match (self.cookie, self.api_key) {
      (Some(cookie), None) => CredentialSecret::Cookies(cookie),
      (None, Some(mut api_key)) => {
        api_key.normalize();
        CredentialSecret::ApiKey(api_key)
      },
      (None, None) => {
        return Err(ArtcraftXCredentialError::MissingSecret { path: source_path });
      },
      (Some(_), Some(_)) => {
        return Err(ArtcraftXCredentialError::AmbiguousSecret { path: source_path });
      },
    };

    if secret.kind() != self.service.kind() {
      return Err(ArtcraftXCredentialError::SecretKindMismatch {
        path: source_path,
        service: self.service,
        found: secret.kind(),
      });
    }

    Ok(AuthCredential {
      id: self.id.unwrap_or_else(CredentialId::generate),
      service: self.service,
      name: self.name,
      secret,
      user_info: self.user_info,
      source_path,
    })
  }

  pub fn from_auth_credential(credential: &AuthCredential) -> Self {
    let (cookie, api_key) = match &credential.secret {
      CredentialSecret::Cookies(cookie) => (Some(cookie.clone()), None),
      CredentialSecret::ApiKey(api_key) => (None, Some(api_key.clone())),
    };
    Self {
      id: Some(credential.id.clone()),
      service: credential.service,
      name: credential.name.clone(),
      cookie,
      api_key,
      user_info: credential.user_info.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use core_types::enums::generation_source::CredentialKind;

  const HAND_WRITTEN_COOKIE_FILE: &str = r#"
    service = "artcraft_cookies"

    [user_info]
    username = "creator123"
    email = "creator@example.com"

    [[cookie.cookies]]
    name = "session"
    value = "abc123"
    domain = "storyteller.ai"
  "#;

  const HAND_WRITTEN_API_KEY_FILE: &str = r#"
    service = "fal_api"

    [api_key]
    api_key = "fal-secret-key-12345"
  "#;

  mod parsing_tests {
    use super::*;

    #[test]
    fn parses_hand_written_cookie_file() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_COOKIE_FILE).unwrap();
      assert_eq!(file.service, GenerationSource::ArtcraftCookies);
      assert_eq!(file.cookie.unwrap().cookie_header(), "session=abc123");
      let user_info = file.user_info.unwrap();
      assert_eq!(user_info.username.as_deref(), Some("creator123"));
      assert_eq!(user_info.email.as_deref(), Some("creator@example.com"));
    }

    #[test]
    fn parses_hand_written_api_key_file_without_prefix() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_API_KEY_FILE).unwrap();
      assert_eq!(file.service, GenerationSource::FalApi);
      assert_eq!(file.api_key.unwrap().api_key, "fal-secret-key-12345");
    }

    #[test]
    fn rejects_unknown_service() {
      let result: Result<CredentialToml, _> =
          toml::from_str("service = \"unknown_service\"\n[api_key]\napi_key = \"x\"\n");
      assert!(result.is_err());
    }
  }

  mod validation_tests {
    use super::*;

    #[test]
    fn cookie_file_becomes_cookie_credential() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_COOKIE_FILE).unwrap();
      let credential = file.into_auth_credential(PathBuf::from("test.toml")).unwrap();
      assert_eq!(credential.kind(), CredentialKind::Cookies);
      assert_eq!(credential.source_path, PathBuf::from("test.toml"));
    }

    #[test]
    fn api_key_prefix_is_computed_on_load() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_API_KEY_FILE).unwrap();
      let credential = file.into_auth_credential(PathBuf::from("test.toml")).unwrap();
      let CredentialSecret::ApiKey(api_key) = &credential.secret else {
        panic!("expected api key secret");
      };
      assert_eq!(api_key.printable_partial_prefix, "fal-s");
    }

    #[test]
    fn missing_secret_is_an_error() {
      let file: CredentialToml = toml::from_str("service = \"fal_api\"").unwrap();
      let result = file.into_auth_credential(PathBuf::from("test.toml"));
      assert!(matches!(result, Err(ArtcraftXCredentialError::MissingSecret { .. })));
    }

    #[test]
    fn both_secrets_is_an_error() {
      let toml_text = r#"
        service = "fal_api"
        [api_key]
        api_key = "x"
        [[cookie.cookies]]
        name = "a"
        value = "b"
        domain = "example.com"
      "#;
      let file: CredentialToml = toml::from_str(toml_text).unwrap();
      let result = file.into_auth_credential(PathBuf::from("test.toml"));
      assert!(matches!(result, Err(ArtcraftXCredentialError::AmbiguousSecret { .. })));
    }

    #[test]
    fn secret_kind_must_match_service_kind() {
      let toml_text = r#"
        service = "fal_api"
        [[cookie.cookies]]
        name = "a"
        value = "b"
        domain = "example.com"
      "#;
      let file: CredentialToml = toml::from_str(toml_text).unwrap();
      let result = file.into_auth_credential(PathBuf::from("test.toml"));
      assert!(matches!(result, Err(ArtcraftXCredentialError::SecretKindMismatch { .. })));
    }
  }

  mod id_tests {
    use super::*;

    #[test]
    fn id_in_file_is_preserved() {
      let toml_text = r#"
        id = "credential_test123"
        service = "fal_api"
        [api_key]
        api_key = "x"
      "#;
      let file: CredentialToml = toml::from_str(toml_text).unwrap();
      let credential = file.into_auth_credential(PathBuf::from("test.toml")).unwrap();
      assert_eq!(credential.id.as_str(), "credential_test123");
    }

    #[test]
    fn missing_id_generates_one() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_API_KEY_FILE).unwrap();
      let credential = file.into_auth_credential(PathBuf::from("test.toml")).unwrap();
      assert!(credential.id.as_str().starts_with("credential_"));
    }
  }

  mod round_trip_tests {
    use super::*;

    #[test]
    fn credential_round_trips_through_toml() {
      let file: CredentialToml = toml::from_str(HAND_WRITTEN_COOKIE_FILE).unwrap();
      let credential = file.into_auth_credential(PathBuf::from("test.toml")).unwrap();
      let rewritten = CredentialToml::from_auth_credential(&credential);
      let toml_text = toml::to_string_pretty(&rewritten).unwrap();
      let reparsed: CredentialToml = toml::from_str(&toml_text).unwrap();
      assert_eq!(reparsed, rewritten);
    }
  }
}
