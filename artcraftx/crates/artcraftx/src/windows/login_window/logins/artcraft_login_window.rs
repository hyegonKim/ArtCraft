use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use once_cell::sync::Lazy;
use reqwest::Url;

// NB(2026-07-31): Artcraft login is temporarily pointed at Storyteller's login
// instead of app.getartcraft.com. Artcraft is a Storyteller frontend backed by
// the same api.storyteller.ai, and its session cookies are scoped to
// `.storyteller.ai`, so a Storyteller login yields cookies valid for Artcraft
// too. The getartcraft.com page can't complete login inside the webview because
// it detects Tauri and routes auth through the Rust HTTP proxy (unavailable in
// this window); the Storyteller login form works directly. Revisit when the
// Artcraft web login is reachable natively.
//
// We use the classic storyteller.ai/login page rather than the studio SPA: the
// studio app runs a GPU-detection step (detect-gpu) on load that can stall
// inside the webview and never redirect off /login, starving the completion
// heuristic. The classic login does a plain post-login redirect.
const DESTINATION_HOSTNAMES: &[&str] =
    &["storyteller.ai", "www.storyteller.ai", "studio.storyteller.ai"];

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://storyteller.ai/").expect("URL should parse")
});

static LOGIN_PAGE_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://storyteller.ai/login").expect("URL should parse")
});

pub struct ArtCraftLoginWindow;

impl LoginWindowSite for ArtCraftLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::ArtCraft
  }

  fn window_title(&self) -> String {
    "Login to Artcraft".to_string()
  }

  fn journey(&self) -> LoginJourney {
    LoginJourney::with_default_pre_navigation()
        .website_entry(WEBSITE_ENTRY_URL.clone())
        .login_page(LOGIN_PAGE_URL.clone())
  }

  fn destination_hostnames(&self) -> &[&str] {
    DESTINATION_HOSTNAMES
  }

  // The Storyteller/Artcraft backend session is the cookie named `session`
  // (see artcraft_client's StorytellerSessionCookie). Its presence on the
  // destination host, after leaving the login page, is the definitive
  // "logged in" signal.
  fn session_cookie_names(&self) -> &[&str] {
    &["session"]
  }
}
