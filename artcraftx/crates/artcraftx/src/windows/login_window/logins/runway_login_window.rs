use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use once_cell::sync::Lazy;
use reqwest::Url;

const DESTINATION_HOSTNAMES: &[&str] = &["app.runwayml.com"];

/// The homepage's login button. Runway's login URL carries volatile
/// tracking/consent query parameters, so we discover it from the page rather
/// than hardcode it.
const LOGIN_LINK_SELECTOR: &str = r#"a[href*="app.runwayml.com/login"]"#;

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://runwayml.com/").expect("URL should parse")
});

pub struct RunwayLoginWindow;

impl LoginWindowSite for RunwayLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::Runway
  }

  fn window_title(&self) -> String {
    "Login to Runway".to_string()
  }

  fn journey(&self) -> LoginJourney {
    LoginJourney::with_default_pre_navigation()
        .website_entry(WEBSITE_ENTRY_URL.clone())
        .login_page_via_link(LOGIN_LINK_SELECTOR)
  }

  fn destination_hostnames(&self) -> &[&str] {
    DESTINATION_HOSTNAMES
  }
}
