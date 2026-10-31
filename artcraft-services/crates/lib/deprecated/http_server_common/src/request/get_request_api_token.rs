use actix_web::HttpRequest;

use crate::request::get_request_header_optional::get_request_header_optional;

const API_TOKEN_HEADER : &str = "AUTHORIZATION";

/// Get the optional API token sent with the request
pub fn get_request_api_token(request: &HttpRequest) -> Option<String> {
  get_request_header_optional(request, API_TOKEN_HEADER)
}
