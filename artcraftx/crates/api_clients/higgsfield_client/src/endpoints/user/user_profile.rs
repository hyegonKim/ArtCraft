//! GET `/fnf/user/profile` — the user's public profile (username, bio,
//! social handles).

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_error::HiggsfieldError;
use serde::Deserialize;
use serde_json::Value;

const PATH: &str = "/fnf/user/profile";

pub struct UserProfileArgs<'a> {
  pub request: UserProfileRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// No parameters; kept for uniformity with the other endpoints.
#[derive(Clone, Debug, Default)]
pub struct UserProfileRequest;

#[derive(Clone, Debug, Deserialize)]
pub struct UserProfileResponse {
  pub username: String,

  #[serde(default)]
  pub avatar_url: Option<String>,

  #[serde(default)]
  pub background_image_url: Option<String>,

  #[serde(default)]
  pub bio: Option<String>,

  #[serde(default)]
  pub headline: Option<String>,

  #[serde(default)]
  pub instagram_handle: Option<String>,

  #[serde(default)]
  pub twitter_handle: Option<String>,

  #[serde(default)]
  pub youtube_handle: Option<String>,

  #[serde(default)]
  pub tiktok_handle: Option<String>,

  #[serde(default)]
  pub location: Option<String>,

  #[serde(default)]
  pub badge: Option<String>,

  #[serde(default)]
  pub badges: Vec<Value>,

  #[serde(default)]
  pub total_projects_views: i64,

  #[serde(default)]
  pub total_projects_likes: i64,

  /// RFC 3339 timestamp.
  #[serde(default)]
  pub created_at: Option<String>,

  /// RFC 3339 timestamp.
  #[serde(default)]
  pub updated_at: Option<String>,
}

pub async fn user_profile(args: UserProfileArgs<'_>) -> Result<UserProfileResponse, HiggsfieldError> {
  send_json_request(HttpMethod::Get, PATH, args.auth, args.host, None::<&()>).await
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Captured, with the username / timestamps scrubbed.
  const PROFILE_RESPONSE: &str = r#"{"username":"testuser","avatar_url":null,"background_image_url":null,"bio":null,"headline":null,"instagram_handle":null,"twitter_handle":null,"youtube_handle":null,"tiktok_handle":null,"location":null,"badge":null,"badges":[],"total_projects_views":0,"total_projects_likes":0,"created_at":"2026-08-31T03:26:11.944804Z","updated_at":"2026-08-31T03:28:31.443066Z"}"#;

  #[test]
  fn profile_response_parses() {
    let response: UserProfileResponse = serde_json::from_str(PROFILE_RESPONSE).unwrap();
    assert_eq!(response.username, "testuser");
    assert!(response.avatar_url.is_none());
    assert!(response.badges.is_empty());
    assert_eq!(response.total_projects_views, 0);
    assert_eq!(response.created_at.as_deref(), Some("2026-08-31T03:26:11.944804Z"));
  }

  #[test]
  fn sparse_profile_parses() {
    let response: UserProfileResponse = serde_json::from_str(r#"{"username":"x"}"#).unwrap();
    assert_eq!(response.username, "x");
    assert_eq!(response.total_projects_likes, 0);
  }

  // ── Live (ignored: needs a real session) ──

  /// Drives the binding off the desktop app's saved Higgsfield login
  /// (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`): mints a
  /// Clerk token from the stored cookies, calls `/fnf/user/profile`, and
  /// prints everything that came back.
  #[tokio::test]
  #[ignore]
  async fn live_user_profile_from_app_credential() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;
    println!(
      "Minted bearer token ({} chars); session {:?}; user-agent {:?}; datadome id present: {}",
      auth.bearer_token.len(),
      session.current_token().await.map(|t| t.session_id().to_string()),
      session.maybe_user_agent(),
      session.maybe_datadome_client_id().is_some(),
    );

    let response = user_profile(UserProfileArgs {
      request: UserProfileRequest,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("\n===== /fnf/user/profile =====\n{:#?}", response);

    assert!(!response.username.is_empty(), "expected a username on the profile");
    assert!(response.created_at.is_some(), "expected a created_at timestamp");
    Ok(())
  }

  #[tokio::test]
  #[ignore]
  async fn live_user_profile() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let response = user_profile(UserProfileArgs {
      request: UserProfileRequest,
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Profile: {:?}", response);
    assert!(!response.username.is_empty());
    Ok(())
  }
}
