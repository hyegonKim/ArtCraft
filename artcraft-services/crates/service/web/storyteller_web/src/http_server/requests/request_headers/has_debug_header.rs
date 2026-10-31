use actix_web::HttpRequest;

use http_server_common::request::get_request_header_optional::get_request_header_optional;

/// Debug requests can get routed to special "debug-only" workers, which can
/// be used to trial new code, run debugging, etc.
const DEBUG_HEADER_NAME : &str = "enable-debug-mode";

/// Get the debug tag for the request
/// This tag can be used to segregate "debug" requests, trigger debugging tools, etc.
pub fn has_debug_header(http_request: &HttpRequest) -> bool {
  let is_debug_request =
      get_request_header_optional(&http_request, DEBUG_HEADER_NAME)
          .is_some();

  is_debug_request
}

#[cfg(test)]
mod tests {
  use actix_web::test::TestRequest;

  use crate::http_server::requests::request_headers::has_debug_header::has_debug_header;

  #[test]
  fn test_has_debug_header() {
    let request = TestRequest::get()
        .insert_header(("enable-debug-mode", "true"))
        .to_http_request();
    assert_eq!(has_debug_header(&request), true);
  }

  #[test]
  fn test_without_debug_header() {
    let request = TestRequest::get()
        .to_http_request();
    assert_eq!(has_debug_header(&request), false);
  }
}
