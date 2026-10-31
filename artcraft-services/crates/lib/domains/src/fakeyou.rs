//! FakeYou domains.

use crate::{Domain, domain};

/// Main website.
pub const FAKEYOU: Domain = domain!("fakeyou.com");

/// HTTP API.
pub const FAKEYOU_API: Domain = domain!("api.fakeyou.com");

/// Engine frontend (also used against production for integration testing).
pub const FAKEYOU_ENGINE: Domain = domain!("engine.fakeyou.com");

pub const FAKEYOU_STAGING: Domain = domain!("staging.fakeyou.com");

/// Local development hosts (resolve to a developer machine).
pub const FAKEYOU_DEV: Domain = domain!("dev.fakeyou.com");
pub const FAKEYOU_DEV_PROXY: Domain = domain!("devproxy.fakeyou.com");

/// Production media CDN. Serves both FakeYou and Storyteller media.
pub const FAKEYOU_CDN: Domain = domain!("cdn-2.fakeyou.com");

/// Development media CDN (a Cloudflare R2 public bucket). Serves both FakeYou
/// and Storyteller development media, like `FAKEYOU_CDN` does in production.
pub const FAKEYOU_DEV_CDN: Domain = domain!("pub-c8a4a5bdbdb048f286b77bdf9f786ff2.r2.dev");

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn website() {
    assert_eq!(FAKEYOU.bare_domain(), "fakeyou.com");
    assert_eq!(FAKEYOU.https_link_without_path(), "https://fakeyou.com");
    assert_eq!(FAKEYOU.https_link_with_root_path(), "https://fakeyou.com/");
  }

  #[test]
  fn api() {
    assert_eq!(FAKEYOU_API.https_link_without_path(), "https://api.fakeyou.com");
  }

  #[test]
  fn engine_and_staging() {
    assert_eq!(FAKEYOU_ENGINE.https_link_without_path(), "https://engine.fakeyou.com");
    assert_eq!(FAKEYOU_STAGING.https_link_without_path(), "https://staging.fakeyou.com");
  }

  #[test]
  fn development() {
    assert_eq!(FAKEYOU_DEV.bare_domain(), "dev.fakeyou.com");
    assert_eq!(FAKEYOU_DEV_PROXY.bare_domain(), "devproxy.fakeyou.com");
  }

  mod cdn {
    use super::*;

    #[test]
    fn production() {
      assert_eq!(FAKEYOU_CDN.bare_domain(), "cdn-2.fakeyou.com");
      assert_eq!(FAKEYOU_CDN.https_link_without_path(), "https://cdn-2.fakeyou.com");
      assert_eq!(FAKEYOU_CDN.https_link_with_root_path(), "https://cdn-2.fakeyou.com/");
    }

    #[test]
    fn development() {
      assert_eq!(FAKEYOU_DEV_CDN.https_link_without_path(), "https://pub-c8a4a5bdbdb048f286b77bdf9f786ff2.r2.dev");
    }

    #[cfg(feature = "url")]
    #[test]
    fn urls() {
      assert_eq!(FAKEYOU_CDN.https_url().as_str(), "https://cdn-2.fakeyou.com/");
      assert_eq!(FAKEYOU_DEV_CDN.https_url().as_str(), "https://pub-c8a4a5bdbdb048f286b77bdf9f786ff2.r2.dev/");
    }
  }
}
