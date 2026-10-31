use crate::creds::openart_cookies::OpenArtCookies;
use crate::creds::openart_session_info::OpenArtSessionInfo;

#[derive(Clone)]
pub struct OpenArtCredentials {
  pub cookies: Option<OpenArtCookies>,
  pub session_info: Option<OpenArtSessionInfo>,
}

impl OpenArtCredentials {
  pub fn from_cookies(cookies: &str) -> Self {
    let cookies = OpenArtCookies::new(cookies.to_string());
    Self { 
      cookies: Some(cookies),
      session_info: None,
    }
  }
}
