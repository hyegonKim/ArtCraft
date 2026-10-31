use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::error::grok_specific_api_error::GrokSpecificApiError;
use cloudflare_errors::filter_cloudflare_errors::filter_cloudflare_errors;
use wreq::StatusCode;

/// We know the response is an error at this point.
/// We're just turning it into the right error.
pub fn categorize_grok_http_error(status_code: StatusCode, maybe_body: Option<&str>) -> GrokError {

  if let Some(body) = maybe_body {
    if let Err(err) = filter_cloudflare_errors(status_code.as_u16(), body) {
      return GrokGenericApiError::CloudflareError(err).into();
    }

    // Match against a lowercased copy but keep `body` (original case) for the
    // error payloads, so callers see exactly what the server sent.
    let lowered = body.to_lowercase();

    // Statsig (x-statsig-id) rejection. The signature must be produced by a
    // real browser DOM; a missing/stale/invalid one yields this 403 notice.
    if is_statsig_rejection(&lowered) {
      return GrokSpecificApiError::StatsigSignatureRejected {
        status_code: status_code.as_u16(),
        body: body.to_string(),
      }.into();
    }

    // Resolution gated to a higher plan, e.g. "1080p video generation is not
    // available on your current plan. Use 720p instead." (Same 403/code 7 shape
    // as the statsig notice, so this must come after `is_statsig_rejection`.)
    if is_resolution_not_allowed(&lowered) {
      return GrokSpecificApiError::VideoResolutionNotAllowed {
        status_code: status_code.as_u16(),
        body: body.to_string(),
      }.into();
    }

    let anti_bot = lowered.contains("anti-bot") || lowered.contains("rejected");

    if anti_bot {
      return GrokSpecificApiError::AutomationBlocked.into();
    }

    match status_code {
      StatusCode::TOO_MANY_REQUESTS => {
        return GrokSpecificApiError::TooManyVideos.into();
      }
      _ => {},
    }

    return GrokGenericApiError::UncategorizedBadResponseWithStatusAndBody {
      status_code,
      body: body.to_string(),
    }.into();
  }

  GrokGenericApiError::UncategorizedBadResponseWithStatus(status_code).into()
}

/// Whether a (lowercased) response body reads like an `x-statsig-id` rejection.
/// Grok's current notice is "This page is out of date. Reload to continue.";
/// the extra needles catch adjacent phrasings without over-matching.
fn is_statsig_rejection(lowered_body: &str) -> bool {
  lowered_body.contains("out of date")
      || lowered_body.contains("reload to continue")
      || lowered_body.contains("x-statsig")
}

/// Whether a (lowercased) response body reads like a plan-gated resolution
/// (e.g. 1080p on a lower tier).
fn is_resolution_not_allowed(lowered_body: &str) -> bool {
  lowered_body.contains("not available on your current plan")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::grok_error::GrokError;

  // Real 403 body observed when sending a stale/invalid x-statsig-id.
  const STATSIG_403_BODY: &str =
    r#"{"error":{"code":7,"message":"This page is out of date. Reload to continue.","details":[]}}"#;

  // Real 403 body observed when requesting 1080p on a lower-tier plan.
  const RESOLUTION_403_BODY: &str =
    r#"{"error":{"code":7,"message":"1080p video generation is not available on your current plan. Use 720p instead.","details":[]}}"#;

  #[test]
  fn statsig_rejection_is_categorized_with_status_and_body() {
    let error = categorize_grok_http_error(StatusCode::FORBIDDEN, Some(STATSIG_403_BODY));
    match error {
      GrokError::ApiSpecific(GrokSpecificApiError::StatsigSignatureRejected { status_code, body }) => {
        assert_eq!(status_code, 403);
        // The full, original-case body is preserved for debugging new notices.
        assert_eq!(body, STATSIG_403_BODY);
        assert!(body.contains("out of date"));
      }
      other => panic!("expected StatsigSignatureRejected, got {:?}", other),
    }
  }

  #[test]
  fn plan_gated_resolution_is_categorized_with_status_and_body() {
    let error = categorize_grok_http_error(StatusCode::FORBIDDEN, Some(RESOLUTION_403_BODY));
    match error {
      GrokError::ApiSpecific(GrokSpecificApiError::VideoResolutionNotAllowed { status_code, body }) => {
        assert_eq!(status_code, 403);
        assert_eq!(body, RESOLUTION_403_BODY);
        assert!(body.contains("not available on your current plan"));
      }
      other => panic!("expected VideoResolutionNotAllowed, got {:?}", other),
    }
  }
}
