/// Which Clerk "frontend API" host mints session tokens for Higgsfield.
///
/// Higgsfield's auth is Clerk; its production frontend API lives at
/// `clerk.higgsfield.ai` (the `iss` claim on every session JWT).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ClerkHost {
  #[default]
  Higgsfield,

  /// A custom base URL, e.g. `http://localhost:8081`. A trailing slash is
  /// tolerated and stripped.
  Custom(String),
}

const PRODUCTION_CLERK_BASE_URL: &str = "https://clerk.higgsfield.ai";

/// Clerk's frontend API is versioned by these two query parameters. The
/// values mirror what the web app's clerk-js sends; Clerk tolerates older
/// ones.
pub(crate) const CLERK_API_VERSION: &str = "2025-04-10";
pub(crate) const CLERK_JS_VERSION: &str = "5.98.0";

impl ClerkHost {
  /// Base URL without a trailing slash; endpoint paths (`/v1/...`) are
  /// appended directly.
  pub fn base_url(&self) -> &str {
    match self {
      Self::Higgsfield => PRODUCTION_CLERK_BASE_URL,
      Self::Custom(base_url) => base_url.trim_end_matches('/'),
    }
  }

  /// Build a full URL for an API path (which must start with `/`), with the
  /// version query parameters attached.
  pub(crate) fn url(&self, path: &str) -> String {
    format!(
      "{}{}?__clerk_api_version={}&_clerk_js_version={}",
      self.base_url(), path, CLERK_API_VERSION, CLERK_JS_VERSION,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn production_url_carries_versions() {
    let url = ClerkHost::Higgsfield.url("/v1/client");
    assert!(url.starts_with("https://clerk.higgsfield.ai/v1/client?"));
    assert!(url.contains("__clerk_api_version=2025-04-10"));
    assert!(url.contains("_clerk_js_version=5.98.0"));
  }

  #[test]
  fn custom_base_url_strips_trailing_slash() {
    let host = ClerkHost::Custom("http://localhost:8081/".to_string());
    assert!(host.url("/v1/client").starts_with("http://localhost:8081/v1/client?"));
  }
}
