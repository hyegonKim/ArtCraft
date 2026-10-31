use actix_web::HttpRequest;

use actix_helpers::extractors::get_request_host::get_request_host;

/// Determine the appropriate host for SET COOKIE headers based on the request.
pub fn get_set_cookie_domain(http_request: &HttpRequest) -> Option<&'static str> {
  let maybe_host = get_request_host(http_request);
  match maybe_host {
    Some("jungle.horse") | Some("api.jungle.horse") => Some(".jungle.horse"),
    Some("fakeyou.com") | Some("api.fakeyou.com") => Some(".fakeyou.com"),
    Some("storyteller.io") | Some("api.storyteller.io") => Some(".storyteller.io"),
    _ => None,
  }
}
