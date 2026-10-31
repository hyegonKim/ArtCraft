use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::LoginJourney;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use once_cell::sync::Lazy;
use reqwest::Url;

const DESTINATION_HOSTNAMES: &[&str] = &["openart.ai", "www.openart.ai"];

static WEBSITE_ENTRY_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://openart.ai/").expect("URL should parse")
});

static LOGIN_PAGE_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://openart.ai/home").expect("URL should parse")
});

pub struct OpenArtLoginWindow;

impl LoginWindowSite for OpenArtLoginWindow {
  fn login_website(&self) -> LoginWebsite {
    LoginWebsite::OpenArt
  }

  fn window_title(&self) -> String {
    "Login to OpenArt".to_string()
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
