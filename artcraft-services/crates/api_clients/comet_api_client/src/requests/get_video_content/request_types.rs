use serde_derive::Deserialize;

/// Error envelope returned by `GET /v1/videos/{id}/content` when the video
/// isn't downloadable (not completed, or failed).
///
/// NB: This error message is the ONLY place CometAPI exposes the underlying
/// provider task status for failed generations (eg. "VIOLATION" for content
/// moderation rejections) — the regular poll endpoint just says "failed".
#[derive(Clone, Debug, Deserialize)]
pub struct VideoContentErrorRawResponse {
  pub error: VideoContentErrorBody,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VideoContentErrorBody {
  /// Eg. "Task is not completed yet, current status: VIOLATION"
  pub message: String,

  /// Eg. "invalid_request_error"
  #[serde(rename = "type")]
  pub error_type: Option<String>,
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Real payload observed from `GET /v1/videos/{id}/content` for a task
  /// that failed content moderation.
  const REAL_VIOLATION_ERROR_RESPONSE: &str = r#"
{"error":{"message":"Task is not completed yet, current status: VIOLATION","type":"invalid_request_error"}}
    "#;

  #[test]
  fn parse_real_violation_error() {
    let response: VideoContentErrorRawResponse = serde_json::from_str(REAL_VIOLATION_ERROR_RESPONSE.trim())
      .expect("should parse");

    assert_eq!(response.error.message, "Task is not completed yet, current status: VIOLATION");
    assert_eq!(response.error.error_type.as_deref(), Some("invalid_request_error"));
  }
}
