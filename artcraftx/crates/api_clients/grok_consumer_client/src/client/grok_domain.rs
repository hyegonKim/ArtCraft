/// The scheme + host requests are sent to, e.g. `https://grok.com`.
const DEFAULT_DOMAIN: &str = "https://grok.com";

/// The Grok website domain endpoint bindings send requests to.
///
/// Overridable per-request so calls can be routed through a proxy, and so
/// tests can point at a local mock server instead of the real website.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum GrokDomain {
  /// The real website, `https://grok.com`.
  #[default]
  DefaultDomain,
  /// A full scheme + host override, e.g. `http://localhost:8080`.
  Custom(String),
}

impl GrokDomain {
  /// The scheme + host to prefix request paths with (no trailing slash).
  pub fn get_domain(&self) -> &str {
    match self {
      Self::DefaultDomain => DEFAULT_DOMAIN,
      Self::Custom(domain) => domain,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_domain_is_the_real_website() {
    assert_eq!(GrokDomain::DefaultDomain.get_domain(), "https://grok.com");
    assert_eq!(GrokDomain::default(), GrokDomain::DefaultDomain);
  }

  #[test]
  fn custom_domain_is_returned_verbatim() {
    let domain = GrokDomain::Custom("http://localhost:8080".to_string());
    assert_eq!(domain.get_domain(), "http://localhost:8080");
  }
}
