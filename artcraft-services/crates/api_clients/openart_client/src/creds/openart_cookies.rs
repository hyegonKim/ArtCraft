
/// The cookies payload is the entire cookie header (potentially many cookies).
#[derive(Clone)]
pub struct OpenArtCookies {
  cookies: String,
}

impl OpenArtCookies {
  pub fn new(cookies: String) -> Self {
    OpenArtCookies { cookies }
  }

  pub fn as_str(&self) -> &str {
    &self.cookies
  }

  pub fn as_bytes(&self) -> &[u8] {
    self.cookies.as_bytes()
  }

  pub fn to_string(&self) -> String {
    self.cookies.clone()
  }
}
