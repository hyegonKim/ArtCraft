
#[derive(Debug, Clone)]
pub struct OpenArtSessionInfo {
  /// This is either a session ID, user ID, or subscription ID.
  /// It is passed as the header `X-USER-ID` in other requests.
  pub sub: Option<String>,
  pub email: Option<String>,
  pub name: Option<String>,
  pub image: Option<String>,
}
