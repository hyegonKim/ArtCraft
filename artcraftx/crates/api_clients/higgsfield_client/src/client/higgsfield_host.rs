/// Which Higgsfield API host to talk to.
///
/// [`HiggsfieldHost::Higgsfield`] is the real service. [`HiggsfieldHost::Custom`]
/// points every request at another base URL — a local mock for tests, a
/// recording proxy, a staging environment, etc.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum HiggsfieldHost {
  /// The production API gateway, `https://fnf-api-gw.higgsfield.ai`.
  #[default]
  Higgsfield,

  /// A custom base URL, e.g. `http://localhost:8080`. A trailing slash is
  /// tolerated and stripped.
  Custom(String),
}

const PRODUCTION_API_BASE_URL: &str = "https://fnf-api-gw.higgsfield.ai";

/// The web app's origin; sent as `origin` / `referer` so requests look like
/// they came from the site, which is what the gateway expects.
pub(crate) const WEB_ORIGIN: &str = "https://higgsfield.ai";

impl HiggsfieldHost {
  /// Base URL without a trailing slash; endpoint paths (`/fnf/...`) are
  /// appended directly.
  pub fn api_base_url(&self) -> &str {
    match self {
      Self::Higgsfield => PRODUCTION_API_BASE_URL,
      Self::Custom(base_url) => base_url.trim_end_matches('/'),
    }
  }

  /// Build a full URL for an API path (which must start with `/`).
  pub(crate) fn url(&self, path: &str) -> String {
    format!("{}{}", self.api_base_url(), path)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn production_base_url() {
    assert_eq!(HiggsfieldHost::Higgsfield.api_base_url(), "https://fnf-api-gw.higgsfield.ai");
    assert_eq!(HiggsfieldHost::Higgsfield.url("/fnf/user"), "https://fnf-api-gw.higgsfield.ai/fnf/user");
  }

  #[test]
  fn custom_base_url_strips_trailing_slash() {
    let host = HiggsfieldHost::Custom("http://localhost:8080/".to_string());
    assert_eq!(host.api_base_url(), "http://localhost:8080");
    assert_eq!(host.url("/fnf/user"), "http://localhost:8080/fnf/user");
  }

  #[test]
  fn default_is_production() {
    assert_eq!(HiggsfieldHost::default(), HiggsfieldHost::Higgsfield);
  }
}
