/// Default WebView User-Agent. Must match the UA that obtained `cf_clearance`
/// and the one the `grok_consumer_client` HTTP client uses (Cloudflare binds
/// `cf_clearance` to the UA).
pub const DEFAULT_USER_AGENT: &str =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:143.0) Gecko/20100101 Firefox/143.0";

/// What a WebView needs so its grok.com page load passes Cloudflare and can run
/// Grok's signer.
///
/// Neither field is folded into the signature itself (see the crate docs); they
/// exist only to make the *page load* succeed. In the app the natural source is
/// the existing xAI login window's WebKit session — same cookie store, same UA.
#[derive(Clone, Debug, Default)]
pub struct BrowserContext {
  /// User-Agent for the WebView. Must match the UA that obtained `cf_clearance`
  /// (Cloudflare binds `cf_clearance` to the UA) and the UA the
  /// `grok_consumer_client` HTTP client uses. `None` keeps the WebView default.
  pub user_agent: Option<String>,

  /// Cookies to seed into the WebView before navigating, as raw
  /// `name=value` pairs scoped to grok.com. Usually left empty when the WebView
  /// already shares the logged-in WebKit data store; supply them explicitly
  /// when driving an isolated WebView.
  pub cookies: Vec<String>,
}

impl BrowserContext {
  /// A context that only pins the User-Agent, relying on the shared WebKit
  /// cookie store for the session.
  pub fn with_user_agent(user_agent: impl Into<String>) -> Self {
    Self {
      user_agent: Some(user_agent.into()),
      cookies: Vec::new(),
    }
  }
}
