use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use crate::windows::login_window::utils::grok_statsig_capture::grok_statsig_init_script;
use grok_consumer_client::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use once_cell::sync::Lazy;
use reqwest::Url;

const DESTINATION_HOSTNAMES: &[&str] = &["grok.com", "www.grok.com"];

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://grok.com/").expect("URL should parse")
});

// The xAI SSO sign-in page that redirects back to grok.com once authenticated.
// accounts.x.ai is in the default auth-flow hostnames, so time spent here counts
// as "still logging in" until the webview returns to grok.com.
static LOGIN_PAGE_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://accounts.x.ai/sign-in?redirect=grok-com&return_to=%2F%3Fq%3D%26reasoningMode%3Dnone%26voice%3Dfalse")
      .expect("URL should parse")
});

/// The Grok website login (stored under the `xai` credential service).
pub struct GrokLoginWindow;

impl LoginWindowSite for GrokLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::XAi
  }

  fn window_title(&self) -> String {
    "Login to Grok".to_string()
  }

  fn journey(&self) -> LoginJourney {
    LoginJourney::with_default_pre_navigation()
        .website_entry(WEBSITE_ENTRY_URL.clone())
        .login_page(LOGIN_PAGE_URL.clone())
  }

  fn destination_hostnames(&self) -> &[&str] {
    DESTINATION_HOSTNAMES
  }

  // grok.com sits behind Cloudflare, whose `cf_clearance` cookie is bound to
  // the User-Agent. `grok_consumer_client` sends every request (and the image
  // websocket upgrade) as Firefox 143 on macOS, so the webview that earns the
  // clearance must present the same UA.
  fn user_agent(&self) -> Option<&'static str> {
    Some(FIREFOX_143_MAC_USER_AGENT)
  }

  // NB: Grok's session rides in the xAI SSO cookies (`sso`/`sso-rw`), but rather
  // than gate the save on exact names we don't fully control, we use the
  // default size/count heuristic — a logged-in grok.com page carries far more
  // cookie data than the login redirect, so a real login is unmistakable, and
  // we never fail to save just because a cookie name changed.

  // Install the passive statsig-capture harness from first page load.
  fn initialization_script(&self) -> Option<String> {
    Some(grok_statsig_init_script())
  }
}
