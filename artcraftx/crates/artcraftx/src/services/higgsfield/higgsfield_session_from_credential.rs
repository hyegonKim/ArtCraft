use crate::credentials::auth_credential::AuthCredential;
use crate::error::artcraftx_error::ArtcraftXError;
use anyhow::anyhow;
use higgsfield_client::session::higgsfield_session::HiggsfieldSession;

/// The one way the app should talk to Higgsfield: a [`HiggsfieldSession`]
/// built from a stored `higgsfield_cookies` credential.
///
/// The session mints and refreshes the Clerk bearer token itself, derives
/// DataDome's `x-datadome-clientid` from the `datadome` cookie, and replays
/// the cookies under the User-Agent the login window recorded — so the
/// requests look like the browser that earned the session. When a call
/// fails with `needs_browser_reauth()`, send the user back through the
/// Higgsfield login window.
pub fn higgsfield_session_from_credential(credential: &AuthCredential) -> Result<HiggsfieldSession, ArtcraftXError> {
  let cookie = credential.cookies().ok_or_else(|| {
    ArtcraftXError::AnyhowError(anyhow!(
      "Credential {} ({}) is not a cookie credential; Higgsfield needs a website login",
      credential.id, credential.service,
    ))
  })?;

  let mut session = HiggsfieldSession::from_cookie_header(cookie.cookie_header());

  if let Some(user_agent) = cookie.user_agent.as_deref() {
    session = session.with_user_agent(user_agent);
  }

  Ok(session)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::auth_credential::CredentialSecret;
  use crate::credentials::cookie_credential::CookieCredential;
  use cookie_store_wrapper::cookie_store::CookieStore;
  use core_types::enums::generation_source::GenerationSource;
  use core_types::identifiers::credential_id::CredentialId;
  use reqwest::Url;
  use std::path::PathBuf;

  fn cookie_credential(user_agent: Option<&str>) -> AuthCredential {
    let origin = Url::parse("https://higgsfield.ai/").unwrap();
    let cookies = CookieStore::from_cookie_header("__client=abc; datadome=dd-client-id; __session=x.y.z", &origin);
    let mut cookie = CookieCredential::new(cookies);
    cookie.user_agent = user_agent.map(str::to_string);
    AuthCredential {
      id: CredentialId::generate(),
      service: GenerationSource::HiggsfieldCookies,
      name: None,
      secret: CredentialSecret::Cookies(cookie),
      user_info: None,
      source_path: PathBuf::from("/tmp/higgsfield_cookies.toml"),
    }
  }

  #[test]
  fn session_carries_recorded_ua_and_datadome_id() {
    let session = higgsfield_session_from_credential(&cookie_credential(Some("Mozilla/5.0 Recorded"))).unwrap();
    assert_eq!(session.maybe_user_agent(), Some("Mozilla/5.0 Recorded"));
    assert_eq!(session.maybe_datadome_client_id(), Some("dd-client-id"));
    assert!(session.cookies().has_clerk_client_cookie());
  }

  #[test]
  fn missing_ua_falls_back_to_client_default() {
    let session = higgsfield_session_from_credential(&cookie_credential(None)).unwrap();
    assert_eq!(session.maybe_user_agent(), None);
  }
}
