use actix_web::error::ResponseError;
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web::HttpResponseBuilder;
use serde::Serialize;

/// Turn error responses into JSON HTTP responses with appropriate status code
pub fn error_to_json_http_response<T>(
  error: &T,
) -> HttpResponse
  where T: ?Sized + Serialize + ResponseError,
{
  let json_body = match serde_json::to_string(error) {
    Ok(json) => json,
    Err(_) => "{}".to_string(),
  };

  HttpResponseBuilder::new(error.status_code())
      .content_type(ContentType::json())
      .body(json_body)
}
