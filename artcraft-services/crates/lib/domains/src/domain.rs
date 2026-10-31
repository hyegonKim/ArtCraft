/// A validated domain name, with its `https://` link forms precomputed at
/// compile time. Build one with the [`domain!`](crate::domain) macro:
///
/// ```
/// use domains::{domain, Domain};
///
/// const EXAMPLE: Domain = domain!("desktop.example.com");
///
/// assert_eq!(EXAMPLE.bare_domain(), "desktop.example.com");
/// assert_eq!(EXAMPLE.https_link_without_path(), "https://desktop.example.com");
/// assert_eq!(EXAMPLE.https_link_with_root_path(), "https://desktop.example.com/");
/// ```
///
/// An invalid literal fails to compile:
///
/// ```compile_fail
/// const BAD: domains::Domain = domains::domain!("https://example.com");
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Domain {
  bare: &'static str,
  https_without_path: &'static str,
  https_with_root_path: &'static str,
}

/// Declares a [`Domain`] from a string literal. Validation runs at compile
/// time: an invalid domain is a build error, never a runtime panic.
#[macro_export]
macro_rules! domain {
  ($domain:literal) => {
    $crate::Domain::__from_macro(
      $domain,
      ::core::concat!("https://", $domain),
      ::core::concat!("https://", $domain, "/"),
    )
  };
}

impl Domain {
  /// Only for the `domain!` macro, which guarantees the three strings agree.
  /// Panics (a compile error in const context) on an invalid domain.
  #[doc(hidden)]
  pub const fn __from_macro(
    bare: &'static str,
    https_without_path: &'static str,
    https_with_root_path: &'static str,
  ) -> Self {
    crate::validate::assert_valid_domain(bare);
    Self { bare, https_without_path, https_with_root_path }
  }

  /// eg. `desktop.getartcraft.com`
  pub const fn bare_domain(&self) -> &'static str {
    self.bare
  }

  /// eg. `https://desktop.getartcraft.com`. This is also the browser `Origin`
  /// header value, so use it for CORS and origin allowlists.
  pub const fn https_link_without_path(&self) -> &'static str {
    self.https_without_path
  }

  /// eg. `https://desktop.getartcraft.com/`
  pub const fn https_link_with_root_path(&self) -> &'static str {
    self.https_with_root_path
  }

  /// eg. `https://desktop.getartcraft.com/` as a `Url`.
  ///
  /// Parsed on first use per domain, then served from a process-wide cache,
  /// so every call returns the same `&'static Url`.
  #[cfg(feature = "url")]
  pub fn https_url(&self) -> &'static url::Url {
    crate::url_cache::https_url(self.bare, self.https_with_root_path)
  }
}

impl std::fmt::Debug for Domain {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Domain({})", self.bare)
  }
}

impl std::fmt::Display for Domain {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.bare)
  }
}

#[cfg(test)]
mod tests {
  use crate::Domain;

  const TEST_DOMAIN: Domain = domain!("desktop.example.com");

  #[test]
  fn bare_domain() {
    assert_eq!(TEST_DOMAIN.bare_domain(), "desktop.example.com");
  }

  #[test]
  fn https_links() {
    assert_eq!(TEST_DOMAIN.https_link_without_path(), "https://desktop.example.com");
    assert_eq!(TEST_DOMAIN.https_link_with_root_path(), "https://desktop.example.com/");
  }

  #[test]
  fn usable_in_const_context() {
    const ORIGINS: &[&str] = &[TEST_DOMAIN.https_link_without_path()];
    assert_eq!(ORIGINS, &["https://desktop.example.com"]);
  }

  #[test]
  fn display_and_debug() {
    assert_eq!(TEST_DOMAIN.to_string(), "desktop.example.com");
    assert_eq!(format!("{TEST_DOMAIN:?}"), "Domain(desktop.example.com)");
  }

  #[cfg(feature = "url")]
  mod url_tests {
    use super::*;

    #[test]
    fn https_url_has_root_path() {
      let url = TEST_DOMAIN.https_url();
      assert_eq!(url.as_str(), "https://desktop.example.com/");
      assert_eq!(url.scheme(), "https");
      assert_eq!(url.host_str(), Some("desktop.example.com"));
      assert_eq!(url.path(), "/");
    }

    #[test]
    fn https_url_is_parsed_once_and_shared() {
      let first = TEST_DOMAIN.https_url();
      let second = TEST_DOMAIN.https_url();
      assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn https_url_is_shared_across_threads() {
      fn assert_send_sync<T: Send + Sync>() {}
      assert_send_sync::<Domain>();
      assert_send_sync::<&'static url::Url>();

      // Borrowed into 'static threads with no Arc or cloning; every thread
      // sees the same parsed instance.
      let main_thread_url = TEST_DOMAIN.https_url();
      let handles: Vec<_> = (0..8)
        .map(|_| std::thread::spawn(|| TEST_DOMAIN.https_url()))
        .collect();
      for handle in handles {
        assert!(std::ptr::eq(handle.join().unwrap(), main_thread_url));
      }
    }

    #[test]
    fn https_url_matches_string_forms() {
      let url = TEST_DOMAIN.https_url();
      assert_eq!(url.as_str(), TEST_DOMAIN.https_link_with_root_path());
      assert_eq!(url.origin().ascii_serialization(), TEST_DOMAIN.https_link_without_path());
    }
  }
}
