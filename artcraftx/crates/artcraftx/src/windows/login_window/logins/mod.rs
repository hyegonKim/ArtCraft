pub mod artcraft_login_window;
pub mod higgsfield_login_window;
pub mod magnific_login_window;
pub mod midjourney_login_window;
pub mod openart_login_window;
pub mod runway_login_window;
pub mod xai_login_window;

use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use crate::windows::login_window::logins::artcraft_login_window::ArtCraftLoginWindow;
use crate::windows::login_window::logins::higgsfield_login_window::HiggsfieldLoginWindow;
use crate::windows::login_window::logins::magnific_login_window::MagnificLoginWindow;
use crate::windows::login_window::logins::midjourney_login_window::MidjourneyLoginWindow;
use crate::windows::login_window::logins::openart_login_window::OpenArtLoginWindow;
use crate::windows::login_window::logins::runway_login_window::RunwayLoginWindow;
use crate::windows::login_window::logins::xai_login_window::GrokLoginWindow;

/// Resolve the [`LoginWindowSite`] driver for a website.
pub fn login_site_for(website: LoginWebsite) -> Box<dyn LoginWindowSite> {
  match website {
    LoginWebsite::ArtCraft => Box::new(ArtCraftLoginWindow),
    LoginWebsite::OpenArt => Box::new(OpenArtLoginWindow),
    LoginWebsite::Higgsfield => Box::new(HiggsfieldLoginWindow),
    LoginWebsite::Runway => Box::new(RunwayLoginWindow),
    LoginWebsite::Magnific => Box::new(MagnificLoginWindow),
    LoginWebsite::Midjourney => Box::new(MidjourneyLoginWindow),
    LoginWebsite::XAi => Box::new(GrokLoginWindow),
  }
}

// Journeys and plans are plain values, so every site's navigation flow is
// validated here without touching the network.
#[cfg(test)]
mod tests {
  use super::*;
  use crate::windows::login_window::login_journey::NavigationAction;

  const ALL_WEBSITES: &[LoginWebsite] = &[
    LoginWebsite::ArtCraft,
    LoginWebsite::OpenArt,
    LoginWebsite::Higgsfield,
    LoginWebsite::Runway,
    LoginWebsite::Magnific,
    LoginWebsite::Midjourney,
    LoginWebsite::XAi,
  ];

  #[test]
  fn every_site_journey_starts_with_a_navigation() {
    for website in ALL_WEBSITES {
      let plan = login_site_for(*website).journey().plan();
      assert!(!plan.is_empty(), "{website} journey is empty");
      assert!(
        matches!(plan.first(), Some(NavigationAction::Navigate(_))),
        "{website} journey must start with a navigation",
      );
    }
  }

  #[test]
  fn every_site_has_cookie_urls_covering_its_destinations() {
    for website in ALL_WEBSITES {
      let site = login_site_for(*website);
      let cookie_hosts: Vec<String> = site.cookie_urls()
          .iter()
          .map(|url| url.host_str().unwrap_or_default().to_string())
          .collect();
      for hostname in site.destination_hostnames() {
        assert!(
          cookie_hosts.iter().any(|host| host == hostname),
          "{website} cookie urls {cookie_hosts:?} miss destination {hostname}",
        );
      }
    }
  }

  #[test]
  fn openart_journey() {
    let plan = login_site_for(LoginWebsite::OpenArt).journey().plan();
    assert_eq!(plan.len(), 3);
    assert_navigates_to(&plan[1], "openart.ai", "/");
    assert_navigates_to(&plan[2], "openart.ai", "/home");
  }

  #[test]
  fn higgsfield_journey_has_no_login_page() {
    let plan = login_site_for(LoginWebsite::Higgsfield).journey().plan();
    assert_eq!(plan.len(), 2);
    assert_navigates_to(&plan[1], "higgsfield.ai", "/");
  }

  #[test]
  fn higgsfield_waits_for_the_clerk_session_cookie() {
    // Without a named session cookie the size/count fallback would fire on
    // the logged-out homepage and close the window before login.
    let site = login_site_for(LoginWebsite::Higgsfield);
    assert_eq!(site.session_cookie_names(), &["__session"]);
  }

  #[test]
  fn cookie_replaying_sites_pin_the_user_agent_their_clients_use() {
    use grok_consumer_client::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
    use higgsfield_client::client::higgsfield_browser_profile::HIGGSFIELD_USER_AGENT;
    use crate::services::midjourney::utils::midjourney_browser_profile::MIDJOURNEY_USER_AGENT;

    assert_eq!(login_site_for(LoginWebsite::Higgsfield).user_agent(), Some(HIGGSFIELD_USER_AGENT));
    assert_eq!(login_site_for(LoginWebsite::XAi).user_agent(), Some(FIREFOX_143_MAC_USER_AGENT));
    assert_eq!(login_site_for(LoginWebsite::Midjourney).user_agent(), Some(MIDJOURNEY_USER_AGENT));
  }

  #[test]
  fn midjourney_journey_has_no_login_page() {
    let plan = login_site_for(LoginWebsite::Midjourney).journey().plan();
    assert_eq!(plan.len(), 2);
    assert_navigates_to(&plan[1], "www.midjourney.com", "/");
  }

  #[test]
  fn midjourney_waits_for_the_auth_token_cookies() {
    let site = login_site_for(LoginWebsite::Midjourney);
    assert_eq!(site.session_cookie_names(), &[
      "__Host-Midjourney.AuthUserTokenV3_i",
      "__Host-Midjourney.AuthUserTokenV3_r",
    ]);
  }

  #[test]
  fn runway_journey_discovers_the_login_link() {
    let plan = login_site_for(LoginWebsite::Runway).journey().plan();
    assert_eq!(plan.len(), 3);
    assert_navigates_to(&plan[1], "runwayml.com", "/");
    match &plan[2] {
      NavigationAction::RunScript(script) => {
        assert!(script.contains("app.runwayml.com/login"));
      }
      other => panic!("expected RunScript, got {other:?}"),
    }
  }

  #[test]
  fn magnific_journey() {
    let plan = login_site_for(LoginWebsite::Magnific).journey().plan();
    assert_eq!(plan.len(), 3);
    assert_navigates_to(&plan[1], "www.magnific.com", "/");
    match &plan[2] {
      NavigationAction::Navigate(url) => {
        assert_eq!(url.host_str(), Some("www.magnific.com"));
        assert_eq!(url.path(), "/log-in");
        assert_eq!(url.query(), Some("client_id=magnific&lang=eno"));
      }
      other => panic!("expected Navigate, got {other:?}"),
    }
  }

  #[test]
  fn grok_journey() {
    let plan = login_site_for(LoginWebsite::XAi).journey().plan();
    assert_eq!(plan.len(), 3);
    assert_navigates_to(&plan[1], "grok.com", "/");
    assert_navigates_to(&plan[2], "accounts.x.ai", "/sign-in");
  }

  fn assert_navigates_to(action: &NavigationAction, host: &str, path: &str) {
    match action {
      NavigationAction::Navigate(url) => {
        assert_eq!(url.host_str(), Some(host));
        assert_eq!(url.path(), path);
      }
      other => panic!("expected Navigate to {host}{path}, got {other:?}"),
    }
  }
}
