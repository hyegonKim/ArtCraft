//! POST `https://clerk.higgsfield.ai/v1/client/sessions/{session_id}/tokens`
//! — mint a fresh session JWT. Cookie-authenticated (`__client`). This is
//! what clerk-js calls in the background roughly once a minute.

use crate::client::clerk_host::ClerkHost;
use crate::client::send_request::{send_clerk_request, HttpMethod, RequestBody};
use crate::credentials::clerk_session_token::ClerkSessionToken;
use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::error::higgsfield_api_error::HiggsfieldApiError;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use serde::{Deserialize, Serialize};

pub struct ClerkSessionTokensArgs<'a> {
  pub request: ClerkSessionTokensRequest,
  pub cookies: &'a HiggsfieldCookies,
  /// The User-Agent of the browser that captured the cookies (see
  /// `HiggsfieldSession::with_user_agent`); `None` uses the pinned default.
  pub maybe_user_agent: Option<&'a str>,
  pub host: &'a ClerkHost,
}

#[derive(Clone, Debug)]
pub struct ClerkSessionTokensRequest {
  /// `sess_...` — from a previous token's `sid` claim or from
  /// [`clerk_client`](crate::endpoints::auth::clerk_client::clerk_client).
  pub session_id: String,

  /// Mint the token for an organization context. Higgsfield's web app
  /// doesn't use this; leave `None`.
  pub maybe_organization_id: Option<String>,
}

pub async fn clerk_session_tokens(args: ClerkSessionTokensArgs<'_>) -> Result<ClerkSessionToken, HiggsfieldError> {
  let session_id = args.request.session_id.trim();
  if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
    return Err(HiggsfieldClientError::InvalidRequest(format!("session_id {:?} is not a Clerk session id", session_id)).into());
  }

  let path = format!("/v1/client/sessions/{}/tokens", session_id);

  // clerk-js sends this form-encoded; an empty form when there's no org.
  let form = TokensForm { organization_id: args.request.maybe_organization_id };

  let raw: TokenEnvelope = send_clerk_request(
    HttpMethod::Post,
    &path,
    args.cookies,
    args.maybe_user_agent,
    args.host,
    RequestBody::Form(&form),
  ).await?;

  let Some(jwt) = raw.jwt() else {
    return Err(HiggsfieldApiError::NoActiveSession { raw_http_body: "token response had no jwt".to_string() }.into());
  };

  Ok(ClerkSessionToken::parse(jwt)?)
}

// ── Wire format ──

#[derive(Serialize)]
struct TokensForm {
  #[serde(skip_serializing_if = "Option::is_none")]
  organization_id: Option<String>,
}

/// `{"object":"token","jwt":"..."}` — but tolerate Clerk's
/// `{"response": {...}}` envelope too.
#[derive(Deserialize)]
struct TokenEnvelope {
  #[serde(default)]
  jwt: Option<String>,

  #[serde(default)]
  response: Option<TokenBody>,
}

#[derive(Deserialize)]
struct TokenBody {
  #[serde(default)]
  jwt: Option<String>,
}

impl TokenEnvelope {
  fn jwt(self) -> Option<String> {
    self.jwt.or_else(|| self.response.and_then(|body| body.jwt))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn form_omits_absent_organization() {
    let form = TokensForm { organization_id: None };
    assert_eq!(serde_json::to_value(&form).unwrap(), serde_json::json!({}));
    let form = TokensForm { organization_id: Some("org_1".into()) };
    assert_eq!(serde_json::to_value(&form).unwrap(), serde_json::json!({"organization_id": "org_1"}));
  }

  #[test]
  fn token_envelope_parses_both_shapes() {
    let flat: TokenEnvelope = serde_json::from_str(r#"{"object":"token","jwt":"a.b.c"}"#).unwrap();
    assert_eq!(flat.jwt().as_deref(), Some("a.b.c"));

    let wrapped: TokenEnvelope = serde_json::from_str(r#"{"response":{"object":"token","jwt":"d.e.f"},"client":null}"#).unwrap();
    assert_eq!(wrapped.jwt().as_deref(), Some("d.e.f"));

    let empty: TokenEnvelope = serde_json::from_str(r#"{"object":"token","jwt":null}"#).unwrap();
    assert_eq!(empty.jwt(), None);
  }

  #[tokio::test]
  async fn bad_session_id_fails_before_any_http() {
    let cookies = HiggsfieldCookies::from_cookie_header("__client=x");
    let host = ClerkHost::Custom("http://127.0.0.1:9".to_string());
    let err = clerk_session_tokens(ClerkSessionTokensArgs {
      request: ClerkSessionTokensRequest { session_id: "../evil".into(), maybe_organization_id: None },
      cookies: &cookies,
      maybe_user_agent: None,
      host: &host,
    }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  // ── Live (ignored: needs captured cookies) ──

  #[tokio::test]
  #[ignore]
  async fn live_mint_session_token() -> anyhow::Result<()> {
    use crate::endpoints::auth::clerk_client::{clerk_client, ClerkClientArgs, ClerkClientRequest};
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_cookies;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let cookies = load_higgsfield_test_cookies()?;
    let client = clerk_client(ClerkClientArgs { request: ClerkClientRequest, cookies: &cookies, maybe_user_agent: None, host: &ClerkHost::Higgsfield })
        .await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let session_id = client.active_session().unwrap().id.clone();

    let token = clerk_session_tokens(ClerkSessionTokensArgs {
      request: ClerkSessionTokensRequest { session_id, maybe_organization_id: None },
      cookies: &cookies,
      maybe_user_agent: None,
      host: &ClerkHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Minted token for {} expiring {}", token.session_id(), token.expires_at());
    assert!(token.is_fresh(chrono::Utc::now(), chrono::Duration::seconds(10)));
    Ok(())
  }
}
