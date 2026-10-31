use std::str::FromStr;

use actix_http::header::HeaderMap;
use actix_http::header::HeaderName;

pub (crate) fn get_ip_from_header(headers: &HeaderMap, header_name: &str) -> Option<String> {
  if let Ok(header_name) = HeaderName::from_str(header_name) {
    headers.get(&header_name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| first_ip_from_list(value))
  } else {
    None
  }
}

// NB: GKE packs in multiple IP addresses into the header:
// Example of header: x-forwarded-for: Some("136.55.189.34, 34.117.9.171")
fn first_ip_from_list(ip_list: &str) -> Option<String> {
  ip_list.split(",")
      .into_iter()
      .map(|ip| ip.trim())
      .find(|ip| !ip.is_empty())
      .map(|ip| ip.to_string())
}

#[cfg(test)]
mod tests {
  use crate::extractors::get_ip_from_header::first_ip_from_list;

  #[test]
  fn test_first_ip() {
    assert_eq!(first_ip_from_list("1.2.3.4"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list("1.2.3.4, 34.117.9.171"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list(", 1.2.3.4"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list("1.2.3.4, "), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list(""), None);
  }
}
