use crate::windows::login_window::utils::grok_statsig_capture::STATSIG_CAPTURE_COOKIE;
use cookie_store_wrapper::cookie_store::{CapturedCookie, CookieStore};
use errors::AnyhowResult;
use reqwest::Url;
use tauri::WebviewWindow;

/// Collect the webview's cookies for the given URLs' domains into a
/// [`CookieStore`].
///
/// We read the *entire* cookie store (`webview.cookies()`) and match domains
/// ourselves rather than using `webview.cookies_for_url()`. wry's
/// `cookies_for_url` filters with an exact `cookie.domain() == url.domain()`
/// comparison, but session cookies are usually set on the parent domain with a
/// leading dot (e.g. `.storyteller.ai`, so they reach `api.storyteller.ai`).
/// That never equals the bare host (`storyteller.ai`), so `cookies_for_url`
/// silently drops exactly the auth cookie we need. `cookies()` returns
/// everything (including HttpOnly cookies), so we filter it correctly here.
pub fn extract_login_window_cookies(
  webview: &WebviewWindow,
  cookie_urls: &[Url],
) -> AnyhowResult<CookieStore> {
  let Some(fallback_url) = cookie_urls.first() else {
    return Ok(CookieStore::empty());
  };

  let hosts: Vec<String> = cookie_urls
      .iter()
      .filter_map(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
      .collect();

  let mut cookie_store = CookieStore::empty();
  for cookie in webview.cookies()?.iter() {
    // The Grok statsig-capture harness stashes its output in a client-only
    // cookie; never persist it into the saved session.
    if cookie.name() == STATSIG_CAPTURE_COOKIE {
      continue;
    }
    if cookie_domain_matches(cookie.domain(), &hosts) {
      cookie_store.insert_captured(
        CapturedCookie {
          name: cookie.name().to_string(),
          value: cookie.value().to_string(),
          maybe_domain: cookie.domain().map(str::to_string),
          maybe_path: cookie.path().map(str::to_string),
        },
        fallback_url,
      );
    }
  }
  Ok(cookie_store)
}

/// Whether a cookie's domain applies to any target host, ignoring a leading
/// dot. A parent-domain cookie (`storyteller.ai`) applies to its subdomains
/// (`studio.storyteller.ai`), and — for our capture purposes — we also keep a
/// subdomain cookie when matching against the parent host. The leading `.`
/// guards against partial-label matches (`evilstoryteller.ai`).
fn cookie_domain_matches(cookie_domain: Option<&str>, hosts: &[String]) -> bool {
  let Some(domain) = cookie_domain else {
    return false;
  };
  let domain = domain.trim_start_matches('.').to_ascii_lowercase();
  if domain.is_empty() {
    return false;
  }
  hosts.iter().any(|host| {
    *host == domain
      || host.ends_with(&format!(".{domain}"))
      || domain.ends_with(&format!(".{host}"))
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn matches_parent_domain_cookie_with_leading_dot() {
    let hosts = vec!["storyteller.ai".to_string()];
    assert!(cookie_domain_matches(Some(".storyteller.ai"), &hosts));
    assert!(cookie_domain_matches(Some("storyteller.ai"), &hosts));
    assert!(cookie_domain_matches(Some("studio.storyteller.ai"), &hosts));
  }

  #[test]
  fn rejects_unrelated_and_lookalike_domains() {
    let hosts = vec!["storyteller.ai".to_string()];
    assert!(!cookie_domain_matches(Some("google.com"), &hosts));
    assert!(!cookie_domain_matches(Some("evilstoryteller.ai"), &hosts));
    assert!(!cookie_domain_matches(None, &hosts));
  }
}
