use actix_web::HttpRequest;

/// Pull out the named request header, if present.
pub fn get_request_header_optional(request: &HttpRequest, header_name: &str) -> Option<String> {
  // TODO: Clean up with transpose() once stable
  let result = request.headers()
      .get(header_name)
      .map(|h| h.to_str());
  match result {
    Some(Ok(header)) => Some(header.to_string()),
    Some(Err(_)) => None,
    None => None,
  }
}