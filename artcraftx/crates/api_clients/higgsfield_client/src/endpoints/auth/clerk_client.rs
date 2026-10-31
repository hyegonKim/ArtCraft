//! GET `https://clerk.higgsfield.ai/v1/client` — Clerk's view of this
//! browser "client": its sessions and, for each, a freshly minted session
//! JWT (`last_active_token`). Cookie-authenticated (`__client`).

use crate::client::clerk_host::ClerkHost;
use crate::client::send_request::{send_clerk_request, HttpMethod, RequestBody};
use crate::credentials::clerk_session_token::ClerkSessionToken;
use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::error::higgsfield_api_error::HiggsfieldApiError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::UserId;
use crate::types::string_enum::string_enum;
use serde::Deserialize;

const PATH: &str = "/v1/client";

string_enum! {
  ClerkSessionStatus {
    Active => "active",
    Expired => "expired",
    Removed => "removed",
    Revoked => "revoked",
    Abandoned => "abandoned",
    Ended => "ended",
    Pending => "pending",
  }
}

pub struct ClerkClientArgs<'a> {
  pub request: ClerkClientRequest,
  pub cookies: &'a HiggsfieldCookies,
  /// The User-Agent of the browser that captured the cookies (see
  /// `HiggsfieldSession::with_user_agent`); `None` uses the pinned default.
  pub maybe_user_agent: Option<&'a str>,
  pub host: &'a ClerkHost,
}

/// No parameters; kept for uniformity with the other endpoints.
#[derive(Clone, Debug, Default)]
pub struct ClerkClientRequest;

#[derive(Clone, Debug)]
pub struct ClerkClientResponse {
  /// Clerk client id (`client_...`).
  pub id: String,

  pub sessions: Vec<ClerkSession>,

  /// Which session the web app was using last.
  pub maybe_last_active_session_id: Option<String>,
}

impl ClerkClientResponse {
  /// The session to mint tokens for: the last active one if it's still
  /// active, else the first active session.
  pub fn active_session(&self) -> Option<&ClerkSession> {
    let is_active = |session: &&ClerkSession| session.status == ClerkSessionStatus::Active;
    self.maybe_last_active_session_id.as_deref()
        .and_then(|id| self.sessions.iter().find(|session| session.id == id))
        .filter(is_active)
        .or_else(|| self.sessions.iter().find(is_active))
  }
}

#[derive(Clone, Debug)]
pub struct ClerkSession {
  /// `sess_...`
  pub id: String,

  pub status: ClerkSessionStatus,

  /// Unix epoch milliseconds.
  pub maybe_expire_at: Option<i64>,

  /// A session JWT Clerk minted while answering this request. Fresh, so it
  /// can be used immediately.
  pub maybe_last_active_token: Option<ClerkSessionToken>,

  pub maybe_user_id: Option<UserId>,
}

/// Fetch the client. Fails with
/// [`HiggsfieldApiError::NoActiveSession`] when Clerk knows the client but
/// it has no active session (signed out / expired).
pub async fn clerk_client(args: ClerkClientArgs<'_>) -> Result<ClerkClientResponse, HiggsfieldError> {
  let raw: ClerkClientEnvelope = send_clerk_request(
    HttpMethod::Get,
    PATH,
    args.cookies,
    args.maybe_user_agent,
    args.host,
    RequestBody::<()>::None,
  ).await?;

  let Some(client) = raw.response else {
    return Err(HiggsfieldApiError::NoActiveSession { raw_http_body: "{\"response\":null}".to_string() }.into());
  };

  let sessions = client.sessions.into_iter().map(|session| ClerkSession {
    id: session.id,
    status: session.status,
    maybe_expire_at: session.expire_at,
    // A token that doesn't parse is treated as absent; the caller will mint
    // one explicitly instead.
    maybe_last_active_token: session.last_active_token
        .and_then(|token| token.jwt)
        .and_then(|jwt| ClerkSessionToken::parse(jwt).ok()),
    maybe_user_id: session.user.map(|user| user.id),
  }).collect::<Vec<_>>();

  let response = ClerkClientResponse {
    id: client.id,
    sessions,
    maybe_last_active_session_id: client.last_active_session_id,
  };

  if response.active_session().is_none() {
    return Err(HiggsfieldApiError::NoActiveSession {
      raw_http_body: format!("client {} has {} session(s), none active", response.id, response.sessions.len()),
    }.into());
  }

  Ok(response)
}

// ── Wire format ──

/// Clerk wraps the client: `{"response": {...}, "client": null}`.
#[derive(Deserialize)]
struct ClerkClientEnvelope {
  #[serde(default)]
  response: Option<RawClerkClient>,
}

#[derive(Deserialize)]
struct RawClerkClient {
  id: String,

  #[serde(default)]
  sessions: Vec<RawClerkSession>,

  #[serde(default)]
  last_active_session_id: Option<String>,
}

#[derive(Deserialize)]
struct RawClerkSession {
  id: String,

  status: ClerkSessionStatus,

  #[serde(default)]
  expire_at: Option<i64>,

  #[serde(default)]
  last_active_token: Option<RawClerkToken>,

  #[serde(default)]
  user: Option<RawClerkUser>,
}

#[derive(Deserialize)]
struct RawClerkToken {
  #[serde(default)]
  jwt: Option<String>,
}

#[derive(Deserialize)]
struct RawClerkUser {
  id: UserId,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::clerk_session_token::tests::fake_clerk_jwt;

  fn envelope(sessions_json: &str, last_active: Option<&str>) -> String {
    let last_active = match last_active {
      Some(id) => format!("\"{id}\""),
      None => "null".to_string(),
    };
    format!(r#"{{"response":{{"object":"client","id":"client_TEST","sessions":[{sessions_json}],"sign_in":null,"sign_up":null,"last_active_session_id":{last_active},"cookie_expires_at":null,"created_at":1788147000000,"updated_at":1788147200000}},"client":null}}"#)
  }

  fn session_json(id: &str, status: &str, jwt: Option<&str>) -> String {
    let token = match jwt {
      Some(jwt) => format!(r#"{{"object":"token","jwt":"{jwt}"}}"#),
      None => "null".to_string(),
    };
    format!(r#"{{"object":"session","id":"{id}","status":"{status}","expire_at":1788751996000,"abandon_at":1790739196000,"last_active_at":1788147196000,"last_active_organization_id":null,"actor":null,"user":{{"id":"user_TESTUSER0000000000000000000","object":"user"}},"public_user_data":{{"identifier":"user@example.com"}},"created_at":1788147196000,"updated_at":1788147196000,"last_active_token":{token}}}"#)
  }

  fn parse(body: &str) -> ClerkClientEnvelope {
    serde_json::from_str(body).unwrap()
  }

  #[test]
  fn envelope_parses_with_token() {
    let jwt = fake_clerk_jwt(&serde_json::json!({"exp": 1_788_147_273, "iat": 1_788_147_213, "sid": "sess_A", "sub": "user_TESTUSER0000000000000000000"}));
    let body = envelope(&session_json("sess_A", "active", Some(&jwt)), Some("sess_A"));
    let raw = parse(&body).response.unwrap();
    assert_eq!(raw.id, "client_TEST");
    assert_eq!(raw.sessions.len(), 1);
    assert_eq!(raw.sessions[0].status, ClerkSessionStatus::Active);
    assert_eq!(raw.sessions[0].last_active_token.as_ref().unwrap().jwt.as_deref(), Some(jwt.as_str()));
    assert_eq!(raw.sessions[0].user.as_ref().unwrap().id.as_str(), "user_TESTUSER0000000000000000000");
    assert_eq!(raw.last_active_session_id.as_deref(), Some("sess_A"));
  }

  #[test]
  fn signed_out_client_has_null_response() {
    let raw = parse(r#"{"response":null,"client":null}"#);
    assert!(raw.response.is_none());
  }

  #[test]
  fn active_session_prefers_last_active_then_first_active() {
    let sessions = vec![
      ClerkSession { id: "sess_expired".into(), status: ClerkSessionStatus::Expired, maybe_expire_at: None, maybe_last_active_token: None, maybe_user_id: None },
      ClerkSession { id: "sess_B".into(), status: ClerkSessionStatus::Active, maybe_expire_at: None, maybe_last_active_token: None, maybe_user_id: None },
      ClerkSession { id: "sess_C".into(), status: ClerkSessionStatus::Active, maybe_expire_at: None, maybe_last_active_token: None, maybe_user_id: None },
    ];

    let response = ClerkClientResponse { id: "client_TEST".into(), sessions: sessions.clone(), maybe_last_active_session_id: Some("sess_C".into()) };
    assert_eq!(response.active_session().unwrap().id, "sess_C");

    // Last active points at a dead session: fall back to the first active.
    let response = ClerkClientResponse { id: "client_TEST".into(), sessions: sessions.clone(), maybe_last_active_session_id: Some("sess_expired".into()) };
    assert_eq!(response.active_session().unwrap().id, "sess_B");

    let response = ClerkClientResponse { id: "client_TEST".into(), sessions: vec![sessions[0].clone()], maybe_last_active_session_id: None };
    assert!(response.active_session().is_none());
  }

  // ── Live (ignored: needs captured cookies) ──

  #[tokio::test]
  #[ignore]
  async fn live_clerk_client() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_cookies;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let cookies = load_higgsfield_test_cookies()?;
    let response = clerk_client(ClerkClientArgs {
      request: ClerkClientRequest,
      cookies: &cookies,
      maybe_user_agent: None,
      host: &ClerkHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let session = response.active_session().unwrap();
    println!("Active session {} (status {}), token expires {:?}", session.id, session.status, session.maybe_last_active_token.as_ref().map(|t| t.expires_at()));
    assert!(session.maybe_last_active_token.is_some());
    Ok(())
  }
}
