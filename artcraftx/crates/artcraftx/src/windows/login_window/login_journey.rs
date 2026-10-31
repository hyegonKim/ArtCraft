use once_cell::sync::Lazy;
use reqwest::Url;

/// Default pre-navigation referrer. Visiting Google first gives the login
/// page a plausible referrer (some providers gate on this).
static DEFAULT_PRE_NAVIGATION_URL: Lazy<Url> = Lazy::new(|| {
  Url::parse("https://google.com").expect("URL should parse")
});

/// How long the injected link-discovery script keeps polling the page for the
/// login link before giving up (milliseconds, page-side).
const FOLLOW_LINK_TIMEOUT_MS: u32 = 30_000;

/// How often the injected link-discovery script re-checks the page for the
/// login link (milliseconds, page-side).
const FOLLOW_LINK_POLL_INTERVAL_MS: u32 = 250;

/// The scripted navigation journey that takes a fresh login window from a
/// cold start to the site's login screen.
///
/// The journey is modeled as named stages that run in order, each optional:
///
/// 1. `pre_navigation` — a referrer page visited before touching the site
///    (e.g. Google).
/// 2. `website_entry` — the site's public entry page (e.g. its homepage).
/// 3. `login_page` — how to reach the login page itself: a fixed URL, or
///    discovered on the entry page when the URL is subject to change
///    (see [`LoginPageTarget`]).
///
/// After the last stage the user signs in by hand and the monitor thread
/// ([`crate::windows::login_window::login_window_thread`]) takes over to
/// capture cookies.
///
/// Future stages slot in as new fields; [`LoginJourney::plan`] flattens
/// whatever is present into the ordered [`NavigationAction`]s the window
/// driver executes, so the driver never changes when stages are added.
#[derive(Clone, Debug)]
pub struct LoginJourney {
  /// Stage 1: referrer page visited before touching the site.
  pub pre_navigation: Option<Url>,
  /// Stage 2: the site's public entry page.
  pub website_entry: Option<Url>,
  /// Stage 3: how to reach the login page.
  pub login_page: Option<LoginPageTarget>,
}

/// How a journey reaches the login page.
#[derive(Clone, Debug)]
pub enum LoginPageTarget {
  /// A fixed, known login URL.
  Url(Url),
  /// The login URL is dynamic (subject to change on the provider's side), so
  /// instead of navigating blindly, wait for the previous stage's page to
  /// load and follow the login link found on the page itself.
  FollowLink {
    /// CSS selector for the anchor that leads to the login page,
    /// e.g. `a[href*="app.example.com/login"]`.
    css_selector: String,
  },
}

/// One concrete step for the login window driver, produced by
/// [`LoginJourney::plan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationAction {
  /// Point the webview at a URL.
  Navigate(Url),
  /// Run a self-contained, idempotent script in the current page. The script
  /// performs any further navigation itself (e.g. following a login link).
  RunScript(String),
}

impl LoginJourney {
  /// An empty journey. Most sites should start from
  /// [`LoginJourney::with_default_pre_navigation`] instead.
  pub fn new() -> Self {
    Self {
      pre_navigation: None,
      website_entry: None,
      login_page: None,
    }
  }

  /// A journey that starts at the default referrer (Google).
  pub fn with_default_pre_navigation() -> Self {
    Self {
      pre_navigation: Some(DEFAULT_PRE_NAVIGATION_URL.clone()),
      ..Self::new()
    }
  }

  /// Set stage 2: the site's public entry page.
  pub fn website_entry(mut self, url: Url) -> Self {
    self.website_entry = Some(url);
    self
  }

  /// Set stage 3 to a fixed login URL.
  pub fn login_page(mut self, url: Url) -> Self {
    self.login_page = Some(LoginPageTarget::Url(url));
    self
  }

  /// Set stage 3 to follow the login link discovered on the previous stage's
  /// page (for sites whose login URL is subject to change).
  pub fn login_page_via_link(mut self, css_selector: &str) -> Self {
    self.login_page = Some(LoginPageTarget::FollowLink {
      css_selector: css_selector.to_string(),
    });
    self
  }

  /// Flatten the present stages, in order, into driver actions.
  pub fn plan(&self) -> Vec<NavigationAction> {
    let mut actions = Vec::new();
    if let Some(url) = &self.pre_navigation {
      actions.push(NavigationAction::Navigate(url.clone()));
    }
    if let Some(url) = &self.website_entry {
      actions.push(NavigationAction::Navigate(url.clone()));
    }
    match &self.login_page {
      Some(LoginPageTarget::Url(url)) => {
        actions.push(NavigationAction::Navigate(url.clone()));
      }
      Some(LoginPageTarget::FollowLink { css_selector }) => {
        actions.push(NavigationAction::RunScript(follow_link_script(css_selector)));
      }
      None => {}
    }
    actions
  }

  /// The site-owned fixed URLs in the journey: the entry page and the login
  /// page when it's a fixed URL. Excludes the pre-navigation referrer (a
  /// third party) and discovered login pages (unknown until runtime). Used
  /// to derive default cookie-capture origins.
  pub fn site_urls(&self) -> Vec<Url> {
    let mut urls = Vec::new();
    if let Some(url) = &self.website_entry {
      urls.push(url.clone());
    }
    if let Some(LoginPageTarget::Url(url)) = &self.login_page {
      urls.push(url.clone());
    }
    urls
  }
}

impl Default for LoginJourney {
  fn default() -> Self {
    Self::new()
  }
}

/// Build the in-page script that waits for the document to be ready, finds
/// the login link by CSS selector, and follows it. Self-contained and
/// idempotent: safe to inject more than once (a document swap between
/// injection and load wipes scripts, so the driver re-injects).
fn follow_link_script(css_selector: &str) -> String {
  let selector_literal =
      serde_json::to_string(css_selector).expect("string serializes to JSON");
  format!(
    r#"(() => {{
  const SELECTOR = {selector_literal};
  const DEADLINE = Date.now() + {FOLLOW_LINK_TIMEOUT_MS};
  const follow = () => {{
    const link = document.querySelector(SELECTOR);
    if (link) {{
      if (link.href) {{
        window.location.assign(link.href);
        return;
      }}
      if (typeof link.click === 'function') {{
        link.click();
        return;
      }}
    }}
    if (Date.now() < DEADLINE) {{
      setTimeout(follow, {FOLLOW_LINK_POLL_INTERVAL_MS});
    }}
  }};
  if (document.readyState === 'loading') {{
    document.addEventListener('DOMContentLoaded', follow);
  }} else {{
    follow();
  }}
}})();"#
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  const ENTRY_URL: &str = "https://example.com/";
  const LOGIN_URL: &str = "https://app.example.com/login";

  fn url(value: &str) -> Url {
    Url::parse(value).unwrap()
  }

  mod plan_tests {
    use super::*;

    #[test]
    fn full_journey_flattens_in_stage_order() {
      let journey = LoginJourney::with_default_pre_navigation()
          .website_entry(url(ENTRY_URL))
          .login_page(url(LOGIN_URL));

      assert_eq!(journey.plan(), vec![
        NavigationAction::Navigate(url("https://google.com")),
        NavigationAction::Navigate(url(ENTRY_URL)),
        NavigationAction::Navigate(url(LOGIN_URL)),
      ]);
    }

    #[test]
    fn stages_are_optional() {
      let journey = LoginJourney::new().website_entry(url(ENTRY_URL));

      assert_eq!(journey.plan(), vec![
        NavigationAction::Navigate(url(ENTRY_URL)),
      ]);
    }

    #[test]
    fn empty_journey_has_empty_plan() {
      assert!(LoginJourney::new().plan().is_empty());
    }

    #[test]
    fn follow_link_login_page_becomes_a_script_action() {
      let journey = LoginJourney::new()
          .website_entry(url(ENTRY_URL))
          .login_page_via_link("a[href*=\"app.example.com/login\"]");

      let plan = journey.plan();
      assert_eq!(plan.len(), 2);
      assert_eq!(plan[0], NavigationAction::Navigate(url(ENTRY_URL)));
      match &plan[1] {
        NavigationAction::RunScript(script) => {
          assert!(script.contains(r#""a[href*=\"app.example.com/login\"]""#));
          assert!(script.contains("document.querySelector"));
        }
        other => panic!("expected RunScript, got {:?}", other),
      }
    }
  }

  mod site_urls_tests {
    use super::*;

    #[test]
    fn excludes_referrer_and_discovered_login_pages() {
      let fixed = LoginJourney::with_default_pre_navigation()
          .website_entry(url(ENTRY_URL))
          .login_page(url(LOGIN_URL));
      assert_eq!(fixed.site_urls(), vec![url(ENTRY_URL), url(LOGIN_URL)]);

      let discovered = LoginJourney::with_default_pre_navigation()
          .website_entry(url(ENTRY_URL))
          .login_page_via_link("a.login");
      assert_eq!(discovered.site_urls(), vec![url(ENTRY_URL)]);
    }
  }

  mod follow_link_script_tests {
    use super::*;

    #[test]
    fn escapes_selector_quotes() {
      let script = follow_link_script(r#"a[href*="login"]"#);
      assert!(script.contains(r#"const SELECTOR = "a[href*=\"login\"]";"#));
    }

    #[test]
    fn waits_for_dom_and_polls() {
      let script = follow_link_script("a.login");
      assert!(script.contains("DOMContentLoaded"));
      assert!(script.contains("setTimeout(follow, 250)"));
      assert!(script.contains("Date.now() + 30000"));
    }
  }
}
