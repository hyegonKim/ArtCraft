use cookie_store_wrapper::cookie_store::{CapturedCookie, CookieStore};
use errors::AnyhowResult;
use once_cell::sync::Lazy;
use reqwest::Url;
use tauri::webview::Cookie;
use tauri::WebviewWindow;

//static WWW_COOKIE_URL: Lazy<Url> = Lazy::new(|| {
//  Url::parse("https://www.grok.com").expect("URL should parse")
//});

static ROOT_COOKIE_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://grok.com").expect("URL should parse")
});

pub fn grok_login_webview_extract_cookies(webview: &WebviewWindow) -> AnyhowResult<CookieStore> {
  let mut cookie_store = CookieStore::empty();
  let cookies = get_all_grok_cookies(webview)?;
  for cookie in cookies.iter() {
    cookie_store.insert_captured(
      CapturedCookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        maybe_domain: cookie.domain().map(str::to_string),
        maybe_path: cookie.path().map(str::to_string),
      },
      &ROOT_COOKIE_URL,
    );
  }
  Ok(cookie_store)
}

fn get_all_grok_cookies(webview: &WebviewWindow) -> AnyhowResult<Vec<Cookie<'_>>> {
  //let www_cookies = webview.cookies_for_url(WWW_COOKIE_URL.clone())?;
  let root_cookies = webview.cookies_for_url(ROOT_COOKIE_URL.clone())?;

  let all_cookies = root_cookies;
  //let mut cookie_names = HashSet::new();

  //for cookie in root_cookies.iter() {
  //  if !cookie_names.contains(cookie.name()) {
  //    cookie_names.insert(cookie.name().to_string());
  //    all_cookies.push(cookie.clone());
  //  }
  //}

  Ok(all_cookies)
}
