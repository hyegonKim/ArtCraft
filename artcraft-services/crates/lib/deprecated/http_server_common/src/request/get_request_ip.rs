use std::str::FromStr;

use actix_http::header::HeaderMap;
use actix_http::header::HeaderName;
use actix_web::dev::ServiceRequest;
use actix_web::HttpRequest;
use log::debug;

pub fn get_request_ip(request: &HttpRequest) -> String {
  let headers = request.headers();
  let maybe_x_forwarded = get_ip_from_header(headers, "x-forwarded-for");
  let maybe_forwarded = get_ip_from_header(headers, "forwarded");

  debug!("(1) x-forwarded-for: {:?}, forwarded: {:?}", maybe_x_forwarded, maybe_forwarded);

  let maybe_ip = maybe_x_forwarded.or(maybe_forwarded);

  match maybe_ip {
    Some(ip_address) => ip_address,
    None => {
      // If we're running without the upstream Rust proxy, we can grab 'x-forarded-for', which is
      // populated by the DigitalOcean load balancer.
      let ip_address_and_port = request.connection_info()
        .remote_addr()
        .unwrap_or("")
        .to_string();
      let ip_address = ip_address_and_port.split(":")
        .collect::<Vec<&str>>()
        .get(0)
        .copied()
        .unwrap_or("")
        .to_string();
      debug!("Forwarded IP address (1): {}", &ip_address);
      ip_address
    },
  }
}

// TODO: De-duplicate
pub fn get_service_request_ip(request: &ServiceRequest) -> String {
  let headers = request.headers();
  let maybe_x_forwarded = get_ip_from_header(headers, "x-forwarded-for");
  let maybe_forwarded = get_ip_from_header(headers, "forwarded");

  debug!("(2) x-forwarded-for: {:?}, forwarded: {:?}", maybe_x_forwarded, maybe_forwarded);

  let maybe_ip = maybe_x_forwarded.or(maybe_forwarded);

  match maybe_ip {
    Some(ip_address) => ip_address,
    None => {
      // If we're running without the upstream Rust proxy, we can grab 'x-forarded-for', which is
      // populated by the DigitalOcean load balancer.
      let ip_address_and_port = request.connection_info()
          .remote_addr()
          .unwrap_or("")
          .to_string();
      let ip_address = ip_address_and_port.split(":")
          .collect::<Vec<&str>>()
          .get(0)
          .copied()
          .unwrap_or("")
          .to_string();
      debug!("(2) Forwarded IP address: {}", &ip_address);
      ip_address
    },
  }
}

fn get_ip_from_header(headers: &HeaderMap, header_name: &str) -> Option<String> {
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
  use crate::request::get_request_ip::first_ip_from_list;

  #[test]
  fn test_first_ip() {
    assert_eq!(first_ip_from_list("1.2.3.4"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list("1.2.3.4, 34.117.9.171"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list(", 1.2.3.4"), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list("1.2.3.4, "), Some("1.2.3.4".to_string()));
    assert_eq!(first_ip_from_list(""), None);
  }
}