use crate::datadome_error::DataDomeError;
use crate::datadome_response_signals::DataDomeResponseSignals;

/// Hosts DataDome serves its challenge / block pages from.
const DATADOME_PAGE_HOSTS: &[&str] = &[
  "captcha-delivery.com",
  "geo.captcha-delivery.com",
  "ct.captcha-delivery.com",
];

/// Decide whether a response is a DataDome response and which kind.
///
/// Returns `None` for 2xx and for anything not attributable to DataDome.
/// DataDome only ever answers with 403 (blocks and challenges alike); the
/// body's `url` decides between challenge and block.
pub fn classify_datadome_response(signals: &DataDomeResponseSignals<'_>) -> Option<DataDomeError> {
  let status = signals.status_code;
  let body = signals.body;

  if (200..300).contains(&status) {
    return None;
  }

  if let Some(url) = extract_challenge_url(body) {
    return Some(classify_by_url(&url));
  }

  let blocked_by_header = signals.maybe_x_dd_b.is_some();
  let body_names_datadome = body.to_ascii_lowercase().contains("datadome");

  if status == 403 && (blocked_by_header || body_names_datadome) {
    return Some(DataDomeError::Unclassified { status_code: status, raw_http_body: body.to_string() });
  }

  None
}

/// The `url` field of DataDome's JSON body, when the body is one.
fn extract_challenge_url(body: &str) -> Option<String> {
  let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
  let url = parsed.get("url")?.as_str()?;
  let is_datadome_page = DATADOME_PAGE_HOSTS.iter().any(|host| url.contains(host));
  is_datadome_page.then(|| url.to_string())
}

fn classify_by_url(url: &str) -> DataDomeError {
  // `t=bv` ("blocked, verified bot") means no challenge is offered.
  let is_hard_block = url.contains("t=bv");
  if is_hard_block {
    return DataDomeError::Blocked { maybe_block_url: Some(url.to_string()) };
  }
  if url.contains("/interstitial/") {
    return DataDomeError::Interstitial { challenge_url: url.to_string() };
  }
  DataDomeError::CaptchaChallenge { challenge_url: url.to_string() }
}

#[cfg(test)]
mod tests {
  use super::*;

  const CAPTCHA_BODY: &str = r#"{"url":"https://geo.captcha-delivery.com/captcha/?initialCid=AHrlqAAAAAMA&hash=A55FBF4311ED37&cid=Yw0QRSl16BFX&t=fe&referer=https%3A%2F%2Fhiggsfield.ai%2F&s=17605&e=8d0e","cid":"Yw0QRSl16BFX"}"#;
  const INTERSTITIAL_BODY: &str = r#"{"url":"https://geo.captcha-delivery.com/interstitial/?initialCid=AHrlqAAAAAMA&hash=A55FBF4311ED37&cid=Yw0QRSl16BFX&referer=https%3A%2F%2Fhiggsfield.ai%2F","cid":"Yw0QRSl16BFX"}"#;
  const BLOCK_BODY: &str = r#"{"url":"https://geo.captcha-delivery.com/captcha/?initialCid=AHrlqAAAAAMA&hash=A55FBF4311ED37&cid=Yw0QRSl16BFX&t=bv&referer=https%3A%2F%2Fhiggsfield.ai%2F","cid":"Yw0QRSl16BFX"}"#;

  fn classify(status: u16, body: &str) -> Option<DataDomeError> {
    classify_datadome_response(&DataDomeResponseSignals::new(status, body))
  }

  #[test]
  fn success_is_never_datadome() {
    assert_eq!(classify(200, CAPTCHA_BODY), None);
  }

  #[test]
  fn captcha_challenge() {
    match classify(403, CAPTCHA_BODY) {
      Some(DataDomeError::CaptchaChallenge { challenge_url }) => assert!(challenge_url.contains("/captcha/")),
      other => panic!("expected CaptchaChallenge, got {:?}", other),
    }
    assert!(classify(403, CAPTCHA_BODY).unwrap().is_challenge());
  }

  #[test]
  fn interstitial() {
    assert!(matches!(classify(403, INTERSTITIAL_BODY), Some(DataDomeError::Interstitial { .. })));
  }

  #[test]
  fn hard_block() {
    let error = classify(403, BLOCK_BODY).unwrap();
    assert!(matches!(error, DataDomeError::Blocked { .. }));
    assert!(error.is_hard_block());
    assert!(!error.is_challenge());
    assert_eq!(error.log_level(), log::Level::Error);
  }

  #[test]
  fn block_header_without_body_detail_is_unclassified() {
    let signals = DataDomeResponseSignals::new(403, "").with_x_dd_b(Some("1"));
    assert!(matches!(classify_datadome_response(&signals), Some(DataDomeError::Unclassified { status_code: 403, .. })));
  }

  #[test]
  fn ordinary_api_errors_are_not_datadome() {
    assert_eq!(classify(403, r#"{"detail":"Forbidden"}"#), None);
    assert_eq!(classify(401, r#"{"detail":"Not authenticated"}"#), None);
    // A JSON body with an unrelated `url` field isn't a challenge.
    assert_eq!(classify(403, r#"{"url":"https://example.com/login"}"#), None);
  }
}
