use crate::classify_datadome_response::classify_datadome_response;
use crate::datadome_error::DataDomeError;
use crate::datadome_response_signals::DataDomeResponseSignals;

/// Status + body shortcut over
/// [`classify_datadome_response`]: `Err` when DataDome produced the
/// response, `Ok(())` for everything else.
pub fn filter_datadome_errors(status_code: u16, body: &str) -> Result<(), DataDomeError> {
  match classify_datadome_response(&DataDomeResponseSignals::new(status_code, body)) {
    Some(error) => Err(error),
    None => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn passes_non_datadome_errors_through() {
    assert!(filter_datadome_errors(200, "ok").is_ok());
    assert!(filter_datadome_errors(403, r#"{"detail":"Forbidden"}"#).is_ok());
  }

  #[test]
  fn catches_challenges() {
    let body = r#"{"url":"https://geo.captcha-delivery.com/captcha/?cid=x&t=fe","cid":"x"}"#;
    assert!(matches!(filter_datadome_errors(403, body), Err(DataDomeError::CaptchaChallenge { .. })));
  }
}
