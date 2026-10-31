//! Refuse to upload first-party assets to Higgsfield. Reference media is
//! downloaded from a source URL and re-uploaded as bytes; media sourced from
//! our own CDNs must never be handed to a third-party provider, so uploads
//! whose source URL is on a first-party domain are rejected client-side.

use crate::error::higgsfield_client_error::HiggsfieldClientError;

/// Domains (including their subdomains) that may not be used as upload
/// sources. These are first-party ArtCraft / FakeYou properties.
pub const BLOCKED_UPLOAD_SOURCE_DOMAINS: &[&str] = &[
  "fakeyou.com",
  "getartcraft.com",
  "artcraft.ai",
  "cdn-2.fakeyou.com", // Subsumed by fakeyou.com; listed for clarity.
];

/// Err( [`HiggsfieldClientError::UploadSourceDomainBlocked`] ) if
/// `source_url`'s host is (or is a subdomain of) a blocked domain.
pub fn check_upload_source_url(source_url: &str) -> Result<(), HiggsfieldClientError> {
  let Some(host) = url_host(source_url) else {
    return Ok(());
  };
  match blocked_domain_for_host(&host) {
    Some(domain) => Err(HiggsfieldClientError::UploadSourceDomainBlocked {
      source_url: source_url.to_string(),
      domain,
    }),
    None => Ok(()),
  }
}

fn blocked_domain_for_host(host: &str) -> Option<&'static str> {
  let host = host.to_ascii_lowercase();
  BLOCKED_UPLOAD_SOURCE_DOMAINS.iter().copied().find(|domain| {
    host == *domain || host.ends_with(&format!(".{domain}"))
  })
}

/// The host of a URL, without scheme, userinfo, port, path or query. `None`
/// when there's no host to speak of (e.g. a relative path or empty string).
fn url_host(url: &str) -> Option<String> {
  let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
  let authority = after_scheme.split(['/', '?', '#']).next()?;
  let host = authority.rsplit_once('@').map(|(_, host)| host).unwrap_or(authority);
  let host = if host.starts_with('[') {
    // Bracketed IPv6: keep everything through the closing bracket.
    host.split_once(']').map(|(inside, _)| &host[..inside.len() + 1]).unwrap_or(host)
  } else {
    host.rsplit_once(':').map(|(host, _)| host).unwrap_or(host)
  };
  if host.is_empty() {
    return None;
  }
  Some(host.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn first_party_domains_are_blocked() {
    for url in [
      "https://fakeyou.com/media/a.png",
      "https://cdn-2.fakeyou.com/media/a/5/a/image_x.png",
      "https://storage.getartcraft.com/media/a.png?query=1",
      "https://artcraft.ai/a.png",
      "https://ARTCRAFT.AI/a.png",
      "http://user:pass@cdn.fakeyou.com:8080/a.png",
    ] {
      assert!(
        matches!(
          check_upload_source_url(url),
          Err(HiggsfieldClientError::UploadSourceDomainBlocked { .. }),
        ),
        "{url} should be blocked",
      );
    }
  }

  #[test]
  fn third_party_urls_pass() {
    for url in [
      "https://cdn.example.com/a.png",
      "https://notfakeyou.com/a.png",
      "https://fakeyou.com.evil.example/a.png",
      "https://higgsfield.ai/a.png",
      "http://[::1]:8080/a.png",
      "not a url",
      "",
    ] {
      assert!(check_upload_source_url(url).is_ok(), "{url:?} should pass");
    }
  }

  #[test]
  fn hosts_extract() {
    assert_eq!(url_host("https://cdn-2.fakeyou.com/a/b.png"), Some("cdn-2.fakeyou.com".to_string()));
    assert_eq!(url_host("https://a.example.com:443?x=1"), Some("a.example.com".to_string()));
    assert_eq!(url_host("http://user@host.example/a"), Some("host.example".to_string()));
    assert_eq!(url_host("http://[::1]:8080/a"), Some("[::1]".to_string()));
    assert_eq!(url_host(""), None);
    assert_eq!(url_host("https://"), None);
  }
}
