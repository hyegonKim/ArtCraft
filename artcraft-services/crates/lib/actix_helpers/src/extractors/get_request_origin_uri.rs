use std::str::FromStr;

use actix_http::header::ORIGIN;
use actix_http::Uri;
use actix_web::HttpRequest;

use errors::AnyhowResult;

pub fn get_request_origin_uri(request: &HttpRequest) -> AnyhowResult<Option<Uri>> {
  Ok(request.headers()
      .get(ORIGIN)
      .map(|origin| origin.to_str())
      .transpose()?
      .filter(|origin| !origin.is_empty())
      .map(|origin| Uri::from_str(origin))
      .transpose()?)
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use actix_http::Uri;
  use actix_web::test::TestRequest;

  use crate::extractors::get_request_origin_uri::get_request_origin_uri;

  #[test]
  fn missing_origin() {
    let request = TestRequest::default()
        .to_http_request();

    let origin = get_request_origin_uri(&request).expect("should be Ok");

    assert_eq!(origin, None);
  }

  #[test]
  fn empty_string_origin() {
    let request = TestRequest::default()
        .insert_header(("Origin", ""))
        .to_http_request();

    let origin = get_request_origin_uri(&request).expect("should be Ok");

    assert_eq!(origin, None);
  }

  #[test]
  fn fakeyou() {
    let request = TestRequest::default()
        .insert_header(("Origin", "https://fakeyou.com"))
        .to_http_request();

    let origin = get_request_origin_uri(&request).expect("should be Ok");

    assert_eq!(origin, Some(Uri::from_str("https://fakeyou.com").unwrap()));
  }
}