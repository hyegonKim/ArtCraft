use crate::classify_cloudflare_response::classify_cloudflare_response;
use crate::cloudflare_error::CloudflareError;
use crate::cloudflare_response_signals::CloudflareResponseSignals;

/// Status + body shortcut over
/// [`classify_cloudflare_response`]: `Err` when the response is a Cloudflare
/// edge response, `Ok(())` for everything else (including non-Cloudflare
/// errors, which the caller classifies itself).
///
/// Prefer the full classifier when the HTTP client exposes response headers
/// — `cf-mitigated` and `server` make the decision exact.
pub fn filter_cloudflare_errors(status_code: u16, body: &str) -> Result<(), CloudflareError> {
  match classify_cloudflare_response(&CloudflareResponseSignals::new(status_code, body)) {
    Some(error) => Err(error),
    None => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn passes_non_cloudflare_errors_through() {
    assert!(filter_cloudflare_errors(200, "ok").is_ok());
    assert!(filter_cloudflare_errors(500, r#"{"detail":"boom"}"#).is_ok());
  }

  #[test]
  fn catches_challenge_pages() {
    assert_eq!(
      filter_cloudflare_errors(403, "<title>Just a moment...</title>"),
      Err(CloudflareError::ChallengeInterstitial403),
    );
  }
}
