use actix_web::http::header::{ToStrError, USER_AGENT};
use actix_web::HttpRequest;
use log::warn;

pub fn get_request_user_agent(request: &HttpRequest) -> Option<&str> {
  let result = try_get_request_user_agent(request);
  result.unwrap_or_else(|err| {
    warn!("Error parsing User-Agent header: {}", err);
    None
  })
}

fn try_get_request_user_agent(request: &HttpRequest) -> Result<Option<&str>, ToStrError> {
  Ok(request.headers()
      .get(USER_AGENT)
      .map(|value| value.to_str())
      .transpose()?
      .filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
  use actix_web::test::TestRequest;

  use crate::extractors::get_request_user_agent::get_request_user_agent;

  #[test]
  fn missing_user_agent() {
    let request = TestRequest::default()
        .to_http_request();

    let origin = get_request_user_agent(&request);

    assert_eq!(origin, None);
  }

  #[test]
  fn empty_string_user_agent() {
    let request = TestRequest::default()
        .insert_header(("User-Agent", ""))
        .to_http_request();

    let origin = get_request_user_agent(&request);

    assert_eq!(origin, None);
  }

  #[test]
  fn user_agent() {
    let request = TestRequest::default()
        .insert_header(("User-Agent", "Blah Blah"))
        .to_http_request();

    let origin = get_request_user_agent(&request);

    assert_eq!(origin, Some("Blah Blah"));
  }

  #[test]
  fn user_agent_lower_case() {
    let request = TestRequest::default()
        .insert_header(("user-agent", "foo/1.0"))
        .to_http_request();

    let origin = get_request_user_agent(&request);

    assert_eq!(origin, Some("foo/1.0"));
  }
}
