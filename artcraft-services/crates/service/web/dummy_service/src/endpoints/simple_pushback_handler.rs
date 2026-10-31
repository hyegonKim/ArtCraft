use actix_http::StatusCode;
use actix_web::HttpResponse;

/// Send clients 429 Too Many Requests responses so that they back off.
pub async fn simple_pushback_handler() -> HttpResponse {
  HttpResponse::build(StatusCode::TOO_MANY_REQUESTS)
      .content_type("application/json; charset=utf-8")
      .body("{\"success\": false}")
}
