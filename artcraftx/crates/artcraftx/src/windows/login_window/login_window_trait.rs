use core_types::enums::generation_source::GenerationSource;
use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use reqwest::Url;

/// Third-party SSO / identity provider hosts. When the webview is on one of
/// these we know the user is mid auth-flow (and has therefore left the login
/// page), so we never mistake it for the logged-in destination.
const DEFAULT_AUTH_FLOW_HOSTNAMES: &[&str] = &[
  "accounts.google.com",
  "accounts.youtube.com",
  "accounts.x.ai",
  "auth.openai.com",
  "appleid.apple.com",
  "login.live.com",
  "www.facebook.com",
  "facebook.com",
  "github.com",
  "discord.com",
  "x.com",
  "twitter.com",
];

/// A website whose login flow the app can drive in an embedded webview.
///
/// Implementors describe the navigation [`LoginJourney`] to the site's login
/// screen and how to recognize a completed login. Sensible defaults are
/// provided for the SSO host list, cookie origins, and the completion
/// heuristics; a site only overrides what differs.
///
/// The driver executes [`journey`]'s plan, then the monitor thread watches
/// for [`destination_hostnames`] plus cookie signals to decide when the user
/// has finished.
pub trait LoginWindowSite: Send + Sync {
  /// Which website this drives (also selects the credential service).
  fn login_website(&self) -> LoginWebsite;

  /// Human-facing window title (e.g. "Login to Artcraft").
  fn window_title(&self) -> String;

  /// The navigation journey from a cold window to the site's login screen.
  fn journey(&self) -> LoginJourney;

  /// Hosts that indicate the user has reached the logged-in destination.
  /// e.g. `["app.getartcraft.com"]`. Note the login page is often on one of
  /// these too, so this is combined with cookie heuristics — see
  /// [`crate::windows::login_window::login_window_thread`].
  fn destination_hostnames(&self) -> &[&str];

  /// The credential service that a successful login is saved under.
  fn credential_service(&self) -> GenerationSource {
    self.login_website().credential_service()
  }

  /// An optional User-Agent override for the login webview. Return `Some` when
  /// the site's sign-in misbehaves under the default WKWebView UA (e.g. Google
  /// forcing a passkey step-up), or when the captured cookies must later be
  /// replayed by an HTTP client under a specific UA (Cloudflare `cf_clearance`
  /// is UA-bound, so the two must match). Defaults to the platform default.
  fn user_agent(&self) -> Option<&'static str> {
    None
  }

  /// SSO / identity-provider hosts that mean "still logging in".
  fn auth_flow_hostnames(&self) -> &[&str] {
    DEFAULT_AUTH_FLOW_HOSTNAMES
  }

  /// An optional JS initialization script injected before page scripts on every
  /// load of the login webview. Used by Grok to install the passive statsig
  /// capture harness. Defaults to none.
  fn initialization_script(&self) -> Option<String> {
    None
  }

  /// URLs whose cookies we read once the user appears logged in. Defaults to
  /// the origins of the journey's site URLs plus the destination hostnames
  /// (which covers sites whose login page is discovered at runtime).
  fn cookie_urls(&self) -> Vec<Url> {
    let mut urls: Vec<Url> = Vec::new();
    for url in self.journey().site_urls() {
      let origin = origin_url(&url);
      if !urls.contains(&origin) {
        urls.push(origin);
      }
    }
    for hostname in self.destination_hostnames() {
      if let Ok(origin) = Url::parse(&format!("https://{hostname}/")) {
        if !urls.contains(&origin) {
          urls.push(origin);
        }
      }
    }
    urls
  }

  /// Cookie names that, if present, strongly indicate a live session. When
  /// non-empty, at least one must be present before we consider login done.
  /// Empty by default (we fall back to size/count heuristics).
  fn session_cookie_names(&self) -> &[&str] {
    &[]
  }

  /// Minimum number of cookies expected once logged in.
  fn min_cookie_count(&self) -> usize {
    3
  }

  /// Minimum approximate cookie-header length (chars) expected once logged in.
  /// Session/auth tokens are large, so a real login pushes well past a page's
  /// baseline analytics cookies.
  fn min_cookie_char_length(&self) -> usize {
    800
  }
}

/// The scheme+host+port origin of a URL, with the path/query stripped — the
/// form Tauri's `cookies_for_url` expects.
pub fn origin_url(url: &Url) -> Url {
  let mut origin = url.clone();
  origin.set_path("/");
  origin.set_query(None);
  origin.set_fragment(None);
  origin
}
