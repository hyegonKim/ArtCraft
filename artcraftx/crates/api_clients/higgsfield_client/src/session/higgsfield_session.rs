use crate::client::clerk_host::ClerkHost;
use crate::client::higgsfield_host::HiggsfieldHost;
use crate::credentials::clerk_session_token::ClerkSessionToken;
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::credentials::higgsfield_cookies::HiggsfieldCookies;
use crate::endpoints::auth::clerk_client::{clerk_client, ClerkClientArgs, ClerkClientRequest};
use crate::endpoints::auth::clerk_session_tokens::{clerk_session_tokens, ClerkSessionTokensArgs, ClerkSessionTokensRequest};
use crate::error::higgsfield_error::HiggsfieldError;
use chrono::{DateTime, Duration, Utc};
use datadome_mitigation::client_id::datadome_client_id::client_id_from_cookie_header;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mint a new token when the current one has less than this left. Clerk
/// tokens live ~60s, so this keeps a comfortable runway for the request.
const DEFAULT_REFRESH_MARGIN_SECONDS: i64 = 15;

/// A logged-in Higgsfield session that manages its own bearer token.
///
/// Build one from the browser's cookies (the long-lived credential), then
/// call endpoints through it — every call gets a fresh Clerk JWT, minted or
/// refreshed on demand, and a `401` triggers one re-mint and retry.
///
/// Bot protection is handled too: requests present the User-Agent that
/// captured the cookies (see [`Self::with_user_agent`]) and DataDome's
/// `x-datadome-clientid` derived from the `datadome` cookie, so replayed
/// sessions look like the browser that earned them. A Cloudflare or DataDome
/// challenge surfaces as an error whose `needs_browser_reauth()` is true —
/// the cue to send the user back through the login window.
///
/// ```ignore
/// let session = HiggsfieldSession::from_cookies(HiggsfieldCookies::from_cookie_header(cookie_header));
/// let enqueued = session.nano_banana_pro(NanoBananaProRequest::text_to_image("a cat", NanoBananaProAspectRatio::Square1x1, NanoBananaProResolution::OneK)).await?;
/// let job = session.wait_for_job(&enqueued.job_ids()[0], WaitForJobOptions::default()).await?;
/// println!("{:?}", job.result_url());
/// ```
///
/// Cheap to clone; clones share the cached token. Refreshes are
/// single-flight: concurrent callers wait for one mint rather than each
/// minting their own.
#[derive(Clone)]
pub struct HiggsfieldSession {
  cookies: HiggsfieldCookies,
  maybe_datadome_client_id: Option<String>,
  maybe_user_agent: Option<String>,
  api_host: HiggsfieldHost,
  clerk_host: ClerkHost,
  refresh_margin: Duration,
  state: Arc<Mutex<SessionState>>,
}

#[derive(Default)]
struct SessionState {
  /// The most recent token, fresh or not.
  maybe_token: Option<ClerkSessionToken>,

  /// Known Clerk session id, so refreshes can skip the client lookup.
  maybe_session_id: Option<String>,
}

impl HiggsfieldSession {
  /// From captured browser cookies. If they include a `__session` JWT it
  /// seeds the token (even expired, it tells us the session id).
  pub fn from_cookies(cookies: HiggsfieldCookies) -> Self {
    let seed = cookies.maybe_session_jwt()
        .and_then(|jwt| ClerkSessionToken::parse(jwt).ok());
    let maybe_session_id = seed.as_ref().map(|token| token.session_id().to_string());

    // DataDome expects the header to echo its cookie; a browser's JS does
    // this automatically, so a replayed session must too.
    let maybe_datadome_client_id = client_id_from_cookie_header(cookies.as_header_value());

    Self {
      cookies,
      maybe_datadome_client_id,
      maybe_user_agent: None,
      api_host: HiggsfieldHost::Higgsfield,
      clerk_host: ClerkHost::Higgsfield,
      refresh_margin: Duration::seconds(DEFAULT_REFRESH_MARGIN_SECONDS),
      state: Arc::new(Mutex::new(SessionState { maybe_token: seed, maybe_session_id })),
    }
  }

  /// From a `cookie` header value; see [`Self::from_cookies`].
  pub fn from_cookie_header(cookie_header: impl Into<String>) -> Self {
    Self::from_cookies(HiggsfieldCookies::from_cookie_header(cookie_header))
  }

  /// Override the `x-datadome-clientid` sent with gateway requests. By
  /// default it's derived from the `datadome` cookie, which is what the
  /// browser sends; only override if you captured a different value.
  pub fn with_datadome_client_id(mut self, datadome_client_id: impl Into<String>) -> Self {
    self.maybe_datadome_client_id = Some(datadome_client_id.into().trim().to_string());
    self
  }

  /// The User-Agent of the browser that captured the cookies. Cloudflare's
  /// `cf_clearance` and DataDome's `datadome` cookies are bound to it, so
  /// replaying under a different UA invites a challenge. Without one, the
  /// client's pinned default (`HIGGSFIELD_USER_AGENT`) is used — make sure
  /// the login window uses the same.
  pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
    let user_agent = user_agent.into().trim().to_string();
    self.maybe_user_agent = (!user_agent.is_empty()).then_some(user_agent);
    self
  }

  /// Seed with an already-captured bearer JWT (e.g. from an
  /// `authorization` header). Used until it's near expiry, then refreshed.
  ///
  /// NB: like the other builders, call this before cloning the session —
  /// it replaces the shared state.
  pub fn with_initial_token(mut self, token: ClerkSessionToken) -> Self {
    self.state = Arc::new(Mutex::new(SessionState {
      maybe_session_id: Some(token.session_id().to_string()),
      maybe_token: Some(token),
    }));
    self
  }

  /// Point at other hosts (mocks, proxies, staging).
  pub fn with_hosts(mut self, api_host: HiggsfieldHost, clerk_host: ClerkHost) -> Self {
    self.api_host = api_host;
    self.clerk_host = clerk_host;
    self
  }

  /// How much lifetime a token must have left to be reused.
  pub fn with_refresh_margin(mut self, margin: Duration) -> Self {
    self.refresh_margin = margin;
    self
  }

  pub fn cookies(&self) -> &HiggsfieldCookies {
    &self.cookies
  }

  pub fn api_host(&self) -> &HiggsfieldHost {
    &self.api_host
  }

  pub fn clerk_host(&self) -> &ClerkHost {
    &self.clerk_host
  }

  /// The `x-datadome-clientid` requests carry (derived from the cookies
  /// unless overridden).
  pub fn maybe_datadome_client_id(&self) -> Option<&str> {
    self.maybe_datadome_client_id.as_deref()
  }

  pub fn maybe_user_agent(&self) -> Option<&str> {
    self.maybe_user_agent.as_deref()
  }

  /// A bearer auth that is good for at least the refresh margin, minting
  /// a new token if needed. This is the primitive the endpoint wrappers
  /// use; call it directly to use the raw `endpoints` bindings.
  pub async fn auth(&self) -> Result<HiggsfieldAuth, HiggsfieldError> {
    let token = self.fresh_token(Utc::now()).await?;
    Ok(self.auth_for(token))
  }

  /// Force a new token, regardless of the current one's freshness.
  pub async fn refresh(&self) -> Result<ClerkSessionToken, HiggsfieldError> {
    let mut state = self.state.lock().await;
    self.mint_locked(&mut state).await
  }

  /// The cached token, if any (fresh or not).
  pub async fn current_token(&self) -> Option<ClerkSessionToken> {
    self.state.lock().await.maybe_token.clone()
  }

  /// Run an endpoint call with a fresh auth; on a `401`, mint once more and
  /// retry, since the token may have been revoked or rotated server-side.
  /// Bot-protection refusals are NOT retried — a new token can't pass a
  /// challenge; check `needs_browser_reauth()` on the error instead.
  pub async fn with_auth<T, Fut>(
    &self,
    call: impl Fn(HiggsfieldAuth) -> Fut,
  ) -> Result<T, HiggsfieldError>
  where
    Fut: std::future::Future<Output = Result<T, HiggsfieldError>>,
  {
    let auth = self.auth().await?;
    match call(auth).await {
      Ok(value) => Ok(value),
      Err(err) if err.is_token_rejected() => {
        warn!("Higgsfield request was unauthorized; minting a new session token and retrying once");
        let token = self.refresh().await?;
        call(self.auth_for(token)).await
      }
      Err(err) => Err(err),
    }
  }

  // ── Private ──

  fn auth_for(&self, token: ClerkSessionToken) -> HiggsfieldAuth {
    HiggsfieldAuth {
      bearer_token: token.into_jwt(),
      maybe_cookie_header: Some(self.cookies.as_header_value().to_string()),
      maybe_datadome_client_id: self.maybe_datadome_client_id.clone(),
      maybe_user_agent: self.maybe_user_agent.clone(),
    }
  }

  async fn fresh_token(&self, now: DateTime<Utc>) -> Result<ClerkSessionToken, HiggsfieldError> {
    let mut state = self.state.lock().await;
    if let Some(token) = state.maybe_token.as_ref() {
      if token.is_fresh(now, self.refresh_margin) {
        return Ok(token.clone());
      }
    }
    self.mint_locked(&mut state).await
  }

  /// Mint with the state lock held, so concurrent callers share one mint.
  async fn mint_locked(&self, state: &mut SessionState) -> Result<ClerkSessionToken, HiggsfieldError> {
    let token = match state.maybe_session_id.clone() {
      Some(session_id) => match self.mint_for_session(&session_id).await {
        Ok(token) => token,
        Err(err) if err.is_token_rejected() || matches!(err, HiggsfieldError::Api(crate::error::higgsfield_api_error::HiggsfieldApiError::NoActiveSession { .. })) => {
          // The remembered session may be gone; look the client up again.
          warn!("Minting for Clerk session {} failed ({}); re-discovering the session", session_id, err);
          self.discover_session_token().await?
        }
        Err(err) => return Err(err),
      },
      None => self.discover_session_token().await?,
    };

    info!(
      "Higgsfield session token minted for {} (expires {}, {}s from now)",
      token.session_id(), token.expires_at(), token.remaining(Utc::now()).num_seconds(),
    );

    state.maybe_session_id = Some(token.session_id().to_string());
    state.maybe_token = Some(token.clone());
    Ok(token)
  }

  async fn mint_for_session(&self, session_id: &str) -> Result<ClerkSessionToken, HiggsfieldError> {
    clerk_session_tokens(ClerkSessionTokensArgs {
      request: ClerkSessionTokensRequest { session_id: session_id.to_string(), maybe_organization_id: None },
      cookies: &self.cookies,
      maybe_user_agent: self.maybe_user_agent.as_deref(),
      host: &self.clerk_host,
    }).await
  }

  /// Ask Clerk which session is active. Its reply already includes a fresh
  /// token; use that, minting explicitly only if it didn't.
  async fn discover_session_token(&self) -> Result<ClerkSessionToken, HiggsfieldError> {
    let client = clerk_client(ClerkClientArgs {
      request: ClerkClientRequest,
      cookies: &self.cookies,
      maybe_user_agent: self.maybe_user_agent.as_deref(),
      host: &self.clerk_host,
    }).await?;

    // `clerk_client` guarantees an active session on success.
    let session = client.active_session().expect("clerk_client returns Err without an active session");

    let is_usable = |token: &ClerkSessionToken| token.is_fresh(Utc::now(), self.refresh_margin);
    match session.maybe_last_active_token.as_ref().filter(|token| is_usable(token)) {
      Some(token) => Ok(token.clone()),
      None => self.mint_for_session(&session.id).await,
    }
  }
}

impl std::fmt::Debug for HiggsfieldSession {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("HiggsfieldSession")
        .field("cookies", &self.cookies)
        .field("maybe_user_agent", &self.maybe_user_agent)
        .field("has_datadome_client_id", &self.maybe_datadome_client_id.is_some())
        .field("api_host", &self.api_host)
        .field("clerk_host", &self.clerk_host)
        .field("refresh_margin", &self.refresh_margin)
        .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::clerk_session_token::tests::{fake_clerk_jwt, fake_clerk_token};

  fn cookies_with_session(jwt: &str) -> HiggsfieldCookies {
    HiggsfieldCookies::from_cookie_header(format!("__client=client.cookie.value; __session={jwt}"))
  }

  #[tokio::test]
  async fn seeds_session_id_and_token_from_session_cookie() {
    let jwt = fake_clerk_jwt(&serde_json::json!({"exp": 10, "iat": 5, "sid": "sess_SEED", "sub": "user_x"}));
    let session = HiggsfieldSession::from_cookies(cookies_with_session(&jwt));
    let state = session.state.lock().await;
    assert_eq!(state.maybe_session_id.as_deref(), Some("sess_SEED"));
    assert_eq!(state.maybe_token.as_ref().unwrap().session_id(), "sess_SEED");
  }

  #[tokio::test]
  async fn fresh_seed_token_is_reused_without_network() {
    let far_future = (Utc::now() + Duration::hours(1)).timestamp();
    let token = fake_clerk_token(far_future);
    // Unroutable hosts: any network attempt would fail loudly.
    let session = HiggsfieldSession::from_cookie_header("__client=x")
        .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()))
        .with_initial_token(token.clone());

    let auth = session.auth().await.unwrap();
    assert_eq!(auth.bearer_token, token.jwt());
    assert_eq!(auth.maybe_cookie_header.as_deref(), Some("__client=x"));
  }

  #[tokio::test]
  async fn expired_seed_token_triggers_mint() {
    let expired = fake_clerk_token((Utc::now() - Duration::minutes(5)).timestamp());
    let session = HiggsfieldSession::from_cookie_header("__client=x")
        .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()))
        .with_initial_token(expired);

    // The mint goes to the unroutable Clerk host and fails as a network
    // error — proving a refresh was attempted rather than the stale token
    // being handed out.
    let err = session.auth().await.unwrap_err();
    assert!(!err.is_auth_failure(), "unexpected: {err}");
    assert!(matches!(err, HiggsfieldError::Api(_)), "expected a transport failure, got {err}");
  }

  #[tokio::test]
  async fn missing_client_cookie_fails_before_network() {
    let session = HiggsfieldSession::from_cookie_header("__session=abc")
        .with_hosts(HiggsfieldHost::Custom("http://127.0.0.1:9".into()), ClerkHost::Custom("http://127.0.0.1:9".into()));
    let err = session.auth().await.unwrap_err();
    assert!(err.is_auth_failure());
    assert!(matches!(err, HiggsfieldError::Client(_)));
  }

  #[test]
  fn datadome_client_id_is_derived_from_the_cookie() {
    let session = HiggsfieldSession::from_cookie_header("__client=x; datadome=Yw0QRSl16BFXimBAv1I7~SfJW6aNcwFV");
    assert_eq!(session.maybe_datadome_client_id(), Some("Yw0QRSl16BFXimBAv1I7~SfJW6aNcwFV"));

    let overridden = session.clone().with_datadome_client_id("other");
    assert_eq!(overridden.maybe_datadome_client_id(), Some("other"));

    let without = HiggsfieldSession::from_cookie_header("__client=x");
    assert_eq!(without.maybe_datadome_client_id(), None);
  }

  #[tokio::test]
  async fn user_agent_and_datadome_id_flow_into_auth() {
    let far_future = (Utc::now() + Duration::hours(1)).timestamp();
    let session = HiggsfieldSession::from_cookie_header("__client=x; datadome=dd-id")
        .with_user_agent("  Mozilla/5.0 Test  ")
        .with_initial_token(fake_clerk_token(far_future));
    let auth = session.auth().await.unwrap();
    assert_eq!(auth.maybe_user_agent.as_deref(), Some("Mozilla/5.0 Test"));
    assert_eq!(auth.maybe_datadome_client_id.as_deref(), Some("dd-id"));
  }

  #[test]
  fn debug_is_redacted() {
    let session = HiggsfieldSession::from_cookie_header("__client=super-secret").with_datadome_client_id("dd-secret");
    let debug = format!("{:?}", session);
    assert!(!debug.contains("super-secret"));
    assert!(!debug.contains("dd-secret"));
  }

  // ── Live (ignored: needs captured cookies) ──

  #[tokio::test]
  #[ignore]
  async fn live_session_mints_and_reuses_token() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_test_session()?;
    let first = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let second = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    assert_eq!(first.bearer_token, second.bearer_token, "a fresh token should be reused");

    let refreshed = session.refresh().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("Refreshed token expires {}", refreshed.expires_at());
    assert_ne!(refreshed.jwt(), first.bearer_token);
    Ok(())
  }
}
