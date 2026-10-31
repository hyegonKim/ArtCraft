
#[derive(Clone)]
pub enum FakeYouCredentials {
  ApiToken(ApiTokenCredentials),
  SessionCookie(SessionCookieCredentials),
  //UserAccount(UserAccountCredentials),
}

#[derive(Clone)]
pub struct ApiTokenCredentials {
  pub (crate) token: String,
}

#[derive(Clone)]
pub struct SessionCookieCredentials {
  pub (crate) cookie_value: String,
}

//#[derive(Clone)]
//pub (crate) struct UserAccountCredentials {
//  pub (crate) username_or_email: String,
//  pub (crate) password: String,
//}

impl FakeYouCredentials {
  pub fn from_api_token(token: &str) -> Self {
    Self::ApiToken(ApiTokenCredentials {
      token: token.to_string(),
    })
  }

  /// NB: This is not the full header, just the cookie *value*.
  pub fn from_session_cookie_payload(value: &str) -> Self {
    Self::SessionCookie(SessionCookieCredentials {
      cookie_value: value.to_string(),
    })
  }

  //pub fn from_user_details(username_or_email: &str, password: &str) -> Self {
  //  Self::UserAccount(UserAccountCredentials {
  //    username_or_email: username_or_email.to_string(),
  //    password: password.to_string,
  //  })
  //}
}
