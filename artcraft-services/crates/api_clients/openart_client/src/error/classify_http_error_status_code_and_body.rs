use crate::error::api_error::ApiError;
use crate::error::openart_error::OpenArtError;
use reqwest::StatusCode;

/// This assumes the request failed and returned a non-200.
/// The caller should check.
pub async fn classify_http_error_status_code_and_body(status: StatusCode, body: &str) -> OpenArtError {
  
  let message = body.to_string();

  // TODO: Handle various error messages from the API.

  OpenArtError::Api(ApiError::UncategorizedBadResponse { status_code: status, message })
}
