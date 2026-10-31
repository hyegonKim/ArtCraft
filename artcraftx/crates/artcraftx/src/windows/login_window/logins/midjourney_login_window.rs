use crate::credentials::login_website::LoginWebsite;
use crate::services::midjourney::utils::midjourney_browser_profile::MIDJOURNEY_USER_AGENT;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use once_cell::sync::Lazy;
use reqwest::Url;

const DESTINATION_HOSTNAMES: &[&str] = &["www.midjourney.com", "midjourney.com"];

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://www.midjourney.com/").expect("URL should parse")
});

pub struct MidjourneyLoginWindow;

impl LoginWindowSite for MidjourneyLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::Midjourney
  }

  fn window_title(&self) -> String {
    "Login to Midjourney".to_string()
  }

  /// Midjourney has no discrete login page — the entry page hosts the login.
  fn journey(&self) -> LoginJourney {
    LoginJourney::with_default_pre_navigation()
        .website_entry(WEBSITE_ENTRY_URL.clone())
  }

  fn destination_hostnames(&self) -> &[&str] {
    DESTINATION_HOSTNAMES
  }

  // Google's sign-in forces a passkey step-up under the default WKWebView UA
  // (which the embedded browser can't satisfy). Present a mainstream desktop
  // Safari UA — the SAME one the Midjourney HTTP/websocket calls use — so the
  // captured cf_clearance cookie validates when those calls replay it.
  fn user_agent(&self) -> Option<&'static str> {
    Some(MIDJOURNEY_USER_AGENT)
  }

  // Midjourney's session rides in the AuthUserTokenV3 cookie pair (the same
  // names midjourney_client's cookie_store_has_auth_cookies checks); either
  // appearing in a cookie-cleared window is the definitive login signal.
  fn session_cookie_names(&self) -> &[&str] {
    &[
      "__Host-Midjourney.AuthUserTokenV3_i",
      "__Host-Midjourney.AuthUserTokenV3_r",
    ]
  }
}
