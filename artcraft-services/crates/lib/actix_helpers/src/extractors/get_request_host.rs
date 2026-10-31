use actix_web::http::header::HOST;
use actix_web::HttpRequest;

/// Get the request host
pub fn get_request_host(request: &HttpRequest) -> Option<&str> {
  request.headers()
      .get(HOST)
      .map(|h| h.to_str().ok())
      .flatten()
}
