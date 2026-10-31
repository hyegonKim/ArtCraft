use actix_web::{HttpResponse, HttpResponseBuilder};
use actix_web::http::StatusCode;

/// Convert a string and status code to a JSON error response.
pub fn simple_json_error_response(
  error_reason: &str,
  status_code: StatusCode,
) -> HttpResponse {

  let response = SimpleGenericJsonError {
    success: false,
    error_reason : error_reason.to_string(),
  };

  let body = match serde_json::to_string(&response) {
    Ok(json) => json,
    Err(_) => "{}".to_string(),
  };

  HttpResponseBuilder::new(status_code)
      .content_type("application/json")
      .body(body)
}

#[derive(Serialize)]
struct SimpleGenericJsonError {
  pub success: bool,
  pub error_reason: String,
}

#[cfg(test)]
mod tests {
  use actix_http::body::MessageBody;
  use actix_web::http::StatusCode;

  use crate::error::simple_json_error_response::simple_json_error_response;

  #[test]
  pub fn serialization() {
    let response = simple_json_error_response(
      "foo",
      StatusCode::TOO_MANY_REQUESTS);

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    let body = response.into_body();

    let bytes = body.try_into_bytes().expect("bytes should extract");
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    assert_eq!(body, "{\"success\":false,\"error_reason\":\"foo\"}");
  }
}
