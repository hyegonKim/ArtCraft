use crate::credentials::credential_user_info::CredentialUserInfo;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use base64::Engine;
use cookie_store_wrapper::cookie_store::CookieStore;
use serde_json::Value;

/// JSON claim keys that may carry an email address, most specific first.
const EMAIL_CLAIM_KEYS: &[&str] = &["email", "email_address", "user_email"];

/// JSON claim keys that may carry a human username / handle.
const USERNAME_CLAIM_KEYS: &[&str] = &["username", "preferred_username", "name", "nickname"];

/// Best-effort identity extraction from a freshly captured cookie set.
///
/// Providers frequently drop a JWT (session token, id token, etc.) as one of
/// the cookies. We do not verify it — we only decode the payload to read a
/// username/email so the saved credential can be told apart from other
/// accounts. Returns `None` when nothing identifiable is found.
pub fn extract_user_info_from_cookies(cookie_store: &CookieStore) -> Option<CredentialUserInfo> {
  for (_name, value) in cookie_store.iter_name_values() {
    let Some(claims) = decode_jwt_payload(value) else {
      continue;
    };

    let email = first_string_claim(&claims, EMAIL_CLAIM_KEYS)
        .filter(|value| value.contains('@'));
    let username = first_string_claim(&claims, USERNAME_CLAIM_KEYS);

    if email.is_some() || username.is_some() {
      return Some(CredentialUserInfo { username, email });
    }
  }
  None
}

/// Decode the (unverified) claims of a value that looks like a JWT.
fn decode_jwt_payload(value: &str) -> Option<Value> {
  let mut parts = value.split('.');
  let _header = parts.next()?;
  let payload = parts.next()?;
  let _signature = parts.next()?;
  if parts.next().is_some() {
    return None; // Not a 3-part token.
  }

  let bytes = BASE64_URL_SAFE_NO_PAD.decode(payload).ok()?;
  serde_json::from_slice(&bytes).ok()
}

/// The first non-empty string value among the given claim keys.
fn first_string_claim(claims: &Value, keys: &[&str]) -> Option<String> {
  for key in keys {
    if let Some(value) = claims.get(key).and_then(Value::as_str) {
      let value = value.trim();
      if !value.is_empty() {
        return Some(value.to_string());
      }
    }
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use reqwest::Url;

  // Payload: {"email":"echelon@gmail.com","username":"echelon","exp":1,"iat":1}
  const JWT_WITH_EMAIL: &str = "eyJhbGciOiJIUzI1NiJ9.eyJlbWFpbCI6ImVjaGVsb25AZ21haWwuY29tIiwidXNlcm5hbWUiOiJlY2hlbG9uIiwiZXhwIjoxLCJpYXQiOjF9.abc";

  #[test]
  fn extracts_email_and_username_from_jwt_cookie() {
    let url = site_url();
    let mut store = CookieStore::empty();
    store.insert_named("session", JWT_WITH_EMAIL, &url);
    store.insert_named("analytics", "not-a-jwt", &url);

    let info = extract_user_info_from_cookies(&store).unwrap();
    assert_eq!(info.email.as_deref(), Some("echelon@gmail.com"));
    assert_eq!(info.username.as_deref(), Some("echelon"));
  }

  #[test]
  fn returns_none_without_identifiable_claims() {
    let mut store = CookieStore::empty();
    store.insert_named("plain", "opaque-token", &site_url());
    assert!(extract_user_info_from_cookies(&store).is_none());
  }

  fn site_url() -> Url {
    Url::parse("https://provider.example/").unwrap()
  }
}
