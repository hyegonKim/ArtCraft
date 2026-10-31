use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use once_cell::sync::Lazy;
use reqwest::Url;

const DESTINATION_HOSTNAMES: &[&str] = &["magnific.com", "www.magnific.com"];

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://www.magnific.com/").expect("URL should parse")
});

static LOGIN_PAGE_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://www.magnific.com/log-in?client_id=magnific&lang=eno")
      .expect("URL should parse")
});

pub struct MagnificLoginWindow;

impl LoginWindowSite for MagnificLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::Magnific
  }

  fn window_title(&self) -> String {
    "Login to Magnific".to_string()
  }

  fn journey(&self) -> LoginJourney {
    LoginJourney::with_default_pre_navigation()
        .website_entry(WEBSITE_ENTRY_URL.clone())
        .login_page(LOGIN_PAGE_URL.clone())
  }

  fn destination_hostnames(&self) -> &[&str] {
    DESTINATION_HOSTNAMES
  }
}
