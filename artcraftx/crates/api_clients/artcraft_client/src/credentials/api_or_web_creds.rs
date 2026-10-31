use crate::credentials::storyteller_avt_cookie::StorytellerAvtCookie;
use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::credentials::storyteller_session_cookie::StorytellerSessionCookie;

/// Credentials for endpoints that authenticate as either a web session
/// (cookies) or an API key (`Authorization` header).
#[derive(Clone)]
pub enum ApiOrWebCreds {
  Web(WebCreds),
  Api(ApiKey),
}

/// Website session cookies: the anonymous visitor token and/or the
/// logged-in session cookie.
#[derive(Clone)]
pub struct WebCreds {
  pub maybe_avt: Option<StorytellerAvtCookie>,
  pub maybe_session: Option<StorytellerSessionCookie>,
}

/// An Artcraft API key secret, sent as an `Authorization: Bearer` header.
#[derive(Clone)]
pub struct ApiKey {
  api_key: String,
}

impl ApiOrWebCreds {
  /// The header this credential contributes to a request, as a
  /// `(name, value)` pair. `None` for a web credential with no cookies set.
  pub fn maybe_as_header(&self) -> Option<(&'static str, String)> {
    match self {
      Self::Web(web_creds) => {
        web_creds.maybe_as_cookie_header()
            .map(|value| ("Cookie", value))
      }
      Self::Api(api_key) => {
        Some(("Authorization", api_key.as_authorization_header_value()))
      }
    }
  }
}

impl From<&StorytellerCredentialSet> for ApiOrWebCreds {
  fn from(creds: &StorytellerCredentialSet) -> Self {
    Self::Web(WebCreds::from(creds))
  }
}

impl WebCreds {
  pub fn maybe_as_cookie_header(&self) -> Option<String> {
    let mut cookies = Vec::new();

    if let Some(avt) = &self.maybe_avt {
      cookies.push(avt.as_cookie_header());
    }

    if let Some(session) = &self.maybe_session {
      cookies.push(session.as_cookie_header());
    }

    if cookies.is_empty() {
      None
    } else {
      Some(cookies.join("; "))
    }
  }
}

impl From<&StorytellerCredentialSet> for WebCreds {
  fn from(creds: &StorytellerCredentialSet) -> Self {
    Self {
      maybe_avt: creds.avt.clone(),
      maybe_session: creds.session.clone(),
    }
  }
}

impl ApiKey {
  pub fn new(api_key: String) -> Self {
    Self { api_key }
  }

  pub fn as_authorization_header_value(&self) -> String {
    format!("Bearer {}", self.api_key)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE_KEY: &str = "artcraft_api_55ax0zhd580m598r6n4n7szdwjb2b28sypapvawh";

  mod header_tests {
    use super::*;

    #[test]
    fn api_key_bearer_header() {
      let creds = ApiOrWebCreds::Api(ApiKey::new(SAMPLE_KEY.to_string()));
      assert_eq!(
        creds.maybe_as_header(),
        Some(("Authorization", format!("Bearer {SAMPLE_KEY}"))));
    }

    #[test]
    fn web_creds_cookie_header() {
      let creds = ApiOrWebCreds::Web(WebCreds {
        maybe_avt: Some(StorytellerAvtCookie::new("bob".to_string())),
        maybe_session: Some(StorytellerSessionCookie::new("alice".to_string())),
      });
      assert_eq!(
        creds.maybe_as_header(),
        Some(("Cookie", "visitor=bob; session=alice".to_string())));
    }

    #[test]
    fn empty_web_creds_no_header() {
      let creds = ApiOrWebCreds::Web(WebCreds {
        maybe_avt: None,
        maybe_session: None,
      });
      assert_eq!(creds.maybe_as_header(), None);
    }
  }
}
