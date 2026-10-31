use actix_web::HttpRequest;

use http_server_common::request::get_request_header_optional::get_request_header_optional;

/// The routing tag header can send workloads to particular k8s hosts.
/// This is useful for catching the live logs or intercepting the job.
const ROUTING_TAG_HEADER_NAME : &str = "routing-tag";

/// Get the routing tag for the request.
/// This routing tag can be used to send workloads to particular k8s workers.
pub fn get_routing_tag_header(http_request: &HttpRequest) -> Option<String> {
  let maybe_routing_tag=
      get_request_header_optional(&http_request, ROUTING_TAG_HEADER_NAME)
          .map(|routing_tag| routing_tag.trim().to_string());

  maybe_routing_tag
}

#[cfg(test)]
mod tests {
  use crate::http_server::requests::request_headers::get_routing_tag_header::get_routing_tag_header;
  use actix_web::test::TestRequest;

  #[test]
  fn test_routing_tag_1() {
    let request = TestRequest::get()
        .insert_header(("routing-tag", "foo"))
        .to_http_request();
    assert_eq!(get_routing_tag_header(&request), Some("foo".to_string()));
  }

  #[test]
  fn test_routing_tag_2() {
    let request = TestRequest::post()
        .insert_header(("routing-tag", "bar"))
        .to_http_request();
    assert_eq!(get_routing_tag_header(&request), Some("bar".to_string()));
  }

  #[test]
  fn test_without_routing_tag() {
    let request = TestRequest::get()
        .to_http_request();
    assert_eq!(get_routing_tag_header(&request), None);
  }
}
