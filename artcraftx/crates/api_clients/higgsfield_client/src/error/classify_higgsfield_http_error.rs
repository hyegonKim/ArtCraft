use crate::error::higgsfield_api_error::HiggsfieldApiError;
use crate::error::higgsfield_error::HiggsfieldError;
use cloudflare_errors::classify_cloudflare_response::classify_cloudflare_response;
use cloudflare_errors::cloudflare_response_signals::CloudflareResponseSignals;
use datadome_errors::classify_datadome_response::classify_datadome_response;
use datadome_errors::datadome_response_signals::DataDomeResponseSignals;

/// Everything the transport knows about a response. Header fields are
/// optional so the status + body shortcut still works.
#[derive(Debug, Clone, Default)]
pub struct HttpResponseSignals<'a> {
  pub status_code: u16,
  pub body: &'a str,
  pub maybe_server_header: Option<&'a str>,
  pub maybe_cf_ray: Option<&'a str>,
  pub maybe_cf_mitigated: Option<&'a str>,
  pub maybe_x_datadome: Option<&'a str>,
  pub maybe_x_dd_b: Option<&'a str>,
  /// What was being requested, for log lines (e.g. the URL).
  pub context: &'a str,
}

/// Convert a non-2xx HTTP response into the most specific
/// [`HiggsfieldError`] we can classify. Returns `Ok(())` for 2xx.
///
/// Bot protection is checked first: a DataDome or Cloudflare answer is not
/// an API error and must never be mistaken for one (a challenge page is a
/// 403 too). Those are logged here, at the level the protection crate
/// recommends, so every caller gets consistent logging for free.
pub fn classify_higgsfield_http_response(signals: &HttpResponseSignals<'_>) -> Result<(), HiggsfieldError> {
  let status_code = signals.status_code;
  let body = signals.body;

  if (200..300).contains(&status_code) {
    return Ok(());
  }

  let datadome_signals = DataDomeResponseSignals::new(status_code, body)
      .with_x_datadome(signals.maybe_x_datadome)
      .with_x_dd_b(signals.maybe_x_dd_b);
  if let Some(err) = classify_datadome_response(&datadome_signals) {
    err.log(signals.context);
    return Err(HiggsfieldApiError::DataDome(err).into());
  }

  let cloudflare_signals = CloudflareResponseSignals::new(status_code, body)
      .with_server_header(signals.maybe_server_header)
      .with_cf_ray(signals.maybe_cf_ray)
      .with_cf_mitigated(signals.maybe_cf_mitigated);
  if let Some(err) = classify_cloudflare_response(&cloudflare_signals) {
    err.log(signals.context);
    return Err(HiggsfieldApiError::Cloudflare(err).into());
  }

  let raw = || body.to_string();
  let reason = || extract_error_message(body).unwrap_or_else(|| body.to_string());

  // Content moderation and credit exhaustion can arrive under several 4xx
  // codes; classify by body first for those.
  if (400..500).contains(&status_code) {
    if body_indicates_moderation(body) {
      return Err(HiggsfieldApiError::ContentModerated { reason: reason(), raw_http_body: raw() }.into());
    }
    if status_code == 402 || body_indicates_insufficient_credits(body) {
      return Err(HiggsfieldApiError::InsufficientCredits { reason: reason(), raw_http_body: raw() }.into());
    }
  }

  let error = match status_code {
    400 => HiggsfieldApiError::BadRequest { reason: reason(), raw_http_body: raw() },
    401 => HiggsfieldApiError::Unauthorized { raw_http_body: raw() },
    403 => HiggsfieldApiError::Forbidden { reason: reason(), raw_http_body: raw() },
    404 => HiggsfieldApiError::NotFound { raw_http_body: raw() },
    422 => HiggsfieldApiError::UnprocessableEntity { reason: reason(), raw_http_body: raw() },
    429 => HiggsfieldApiError::RateLimited { raw_http_body: raw() },
    500..=599 => HiggsfieldApiError::ServerError { status_code, raw_http_body: raw() },
    _ => HiggsfieldApiError::UnknownHttpFailure { status_code, raw_http_body: raw() },
  };

  Err(error.into())
}

/// Status + body shortcut over [`classify_higgsfield_http_response`].
pub fn classify_higgsfield_http_error(status_code: u16, maybe_body: Option<&str>) -> Result<(), HiggsfieldError> {
  classify_higgsfield_http_response(&HttpResponseSignals {
    status_code,
    body: maybe_body.unwrap_or(""),
    context: "higgsfield",
    ..Default::default()
  })
}

/// The gateway is FastAPI-flavored: errors usually look like
/// `{"detail": "..."}`, `{"detail": [{"msg": "..."}]}`, `{"message": "..."}`
/// or `{"error": "..."}` / `{"error": {"message": "..."}}`. Clerk's frontend
/// API uses `{"errors": [{"message", "long_message", "code"}]}`.
fn extract_error_message(body: &str) -> Option<String> {
  let parsed: serde_json::Value = serde_json::from_str(body).ok()?;

  if let Some(errors) = parsed.get("errors").and_then(|v| v.as_array()) {
    let messages = errors.iter()
        .filter_map(|item| {
          item.get("long_message").or_else(|| item.get("message")).and_then(|v| v.as_str())
        })
        .collect::<Vec<_>>();
    if !messages.is_empty() {
      return Some(messages.join("; "));
    }
  }

  for key in ["detail", "message", "error"] {
    let Some(value) = parsed.get(key) else { continue };
    match value {
      serde_json::Value::String(s) => return Some(s.clone()),
      serde_json::Value::Object(obj) => {
        if let Some(msg) = obj.get("message").or_else(|| obj.get("msg")).and_then(|v| v.as_str()) {
          return Some(msg.to_string());
        }
      }
      serde_json::Value::Array(items) => {
        let messages = items.iter()
            .filter_map(|item| item.get("msg").and_then(|v| v.as_str()))
            .collect::<Vec<_>>();
        if !messages.is_empty() {
          return Some(messages.join("; "));
        }
      }
      _ => {}
    }
  }

  None
}

fn body_indicates_moderation(body: &str) -> bool {
  let lower = body.to_lowercase();
  lower.contains("moderat")
    || lower.contains("nsfw")
    || lower.contains("content policy")
    || lower.contains("content_policy")
    || lower.contains("safety")
    || lower.contains("prohibited")
}

fn body_indicates_insufficient_credits(body: &str) -> bool {
  let lower = body.to_lowercase();
  (lower.contains("credit") || lower.contains("balance"))
    && (lower.contains("insufficient") || lower.contains("not enough") || lower.contains("enough"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use cloudflare_errors::cloudflare_error::CloudflareError;
  use datadome_errors::datadome_error::DataDomeError;

  // ── Success ──

  #[test]
  fn ok_200_returns_ok() {
    assert!(classify_higgsfield_http_error(200, None).is_ok());
    assert!(classify_higgsfield_http_error(201, Some("{}")).is_ok());
  }

  // ── Bot protection ──

  #[test]
  fn datadome_captcha_is_first_class() {
    let body = r#"{"url":"https://geo.captcha-delivery.com/captcha/?initialCid=abc&hash=def&cid=x&t=fe","cid":"x"}"#;
    let err = classify_higgsfield_http_error(403, Some(body)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::DataDome(DataDomeError::CaptchaChallenge { .. }))));
    assert!(err.needs_browser_reauth());
    assert!(err.is_auth_failure());
    assert!(!err.is_token_rejected());
    assert!(!err.is_retryable());
  }

  #[test]
  fn datadome_hard_block_is_not_reauthable() {
    let body = r#"{"url":"https://geo.captcha-delivery.com/captcha/?cid=x&t=bv","cid":"x"}"#;
    let err = classify_higgsfield_http_error(403, Some(body)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::DataDome(DataDomeError::Blocked { .. }))));
    assert!(!err.needs_browser_reauth());
  }

  #[test]
  fn cloudflare_challenge_is_first_class() {
    let body = "<!DOCTYPE html><title>Just a moment...</title><div id=\"challenge-error-text\"></div>";
    let err = classify_higgsfield_http_error(403, Some(body)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::Cloudflare(CloudflareError::ChallengeInterstitial403))));
    assert!(err.needs_browser_reauth());
    assert!(!err.is_retryable());
  }

  #[test]
  fn cloudflare_challenge_by_header_beats_an_opaque_body() {
    let err = classify_higgsfield_http_response(&HttpResponseSignals {
      status_code: 403,
      body: "",
      maybe_cf_mitigated: Some("challenge"),
      context: "test",
      ..Default::default()
    }).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::Cloudflare(CloudflareError::ChallengeInterstitial403))));
  }

  #[test]
  fn cloudflare_origin_failure_is_retryable() {
    let err = classify_higgsfield_http_response(&HttpResponseSignals {
      status_code: 502,
      body: "",
      maybe_server_header: Some("cloudflare"),
      maybe_cf_ray: Some("8abc-SJC"),
      context: "test",
      ..Default::default()
    }).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::Cloudflare(CloudflareError::BadGateway502))));
    assert!(err.is_retryable());
    assert!(!err.needs_browser_reauth());
  }

  #[test]
  fn an_origin_502_without_cloudflare_markers_is_a_server_error() {
    let err = classify_higgsfield_http_error(502, Some(r#"{"detail":"upstream"}"#)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::ServerError { status_code: 502, .. })));
  }

  // ── Direct status mappings ──

  #[test]
  fn unauthorized_401() {
    let err = classify_higgsfield_http_error(401, Some(r#"{"detail":"Not authenticated"}"#)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::Unauthorized { .. })));
    assert!(err.is_token_rejected());
    assert!(err.is_auth_failure());
    assert!(!err.needs_browser_reauth());
    assert!(!err.is_retryable());
  }

  #[test]
  fn payment_required_402() {
    let err = classify_higgsfield_http_error(402, Some("{}")).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::InsufficientCredits { .. })));
  }

  #[test]
  fn plain_403_is_forbidden() {
    let err = classify_higgsfield_http_error(403, Some(r#"{"detail":"Account suspended"}"#)).unwrap_err();
    match err {
      HiggsfieldError::Api(HiggsfieldApiError::Forbidden { reason, .. }) => assert_eq!(reason, "Account suspended"),
      other => panic!("expected Forbidden, got {:?}", other),
    }
  }

  #[test]
  fn not_found_404() {
    let err = classify_higgsfield_http_error(404, Some(r#"{"detail":"Job not found"}"#)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::NotFound { .. })));
  }

  #[test]
  fn unprocessable_422_joins_fastapi_messages() {
    let body = r#"{"detail":[{"loc":["body","params","prompt"],"msg":"field required","type":"value_error.missing"},{"loc":["body","params","batch_size"],"msg":"ensure this value is less than or equal to 4","type":"value_error"}]}"#;
    let err = classify_higgsfield_http_error(422, Some(body)).unwrap_err();
    match err {
      HiggsfieldError::Api(HiggsfieldApiError::UnprocessableEntity { reason, .. }) => {
        assert_eq!(reason, "field required; ensure this value is less than or equal to 4");
      }
      other => panic!("expected UnprocessableEntity, got {:?}", other),
    }
  }

  #[test]
  fn rate_limited_429_is_retryable() {
    let err = classify_higgsfield_http_error(429, None).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::RateLimited { .. })));
    assert!(err.is_retryable());
  }

  #[test]
  fn server_errors_are_retryable() {
    for status in [500, 502, 503, 504] {
      let err = classify_higgsfield_http_error(status, Some("upstream unavailable")).unwrap_err();
      assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::ServerError { .. })), "status {status}");
      assert!(err.is_retryable());
    }
  }

  #[test]
  fn other_statuses_are_unknown() {
    let err = classify_higgsfield_http_error(418, None).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::UnknownHttpFailure { status_code: 418, .. })));
  }

  // ── Body-driven classification ──

  #[test]
  fn bad_request_400_extracts_detail() {
    let err = classify_higgsfield_http_error(400, Some(r#"{"detail":"Unknown aspect ratio"}"#)).unwrap_err();
    match err {
      HiggsfieldError::Api(HiggsfieldApiError::BadRequest { reason, raw_http_body }) => {
        assert_eq!(reason, "Unknown aspect ratio");
        assert!(raw_http_body.contains("Unknown aspect ratio"));
      }
      other => panic!("expected BadRequest, got {:?}", other),
    }
  }

  #[test]
  fn bad_request_400_unparseable_body_falls_back_to_raw() {
    let err = classify_higgsfield_http_error(400, Some("definitely not json")).unwrap_err();
    match err {
      HiggsfieldError::Api(HiggsfieldApiError::BadRequest { reason, .. }) => assert_eq!(reason, "definitely not json"),
      other => panic!("expected BadRequest, got {:?}", other),
    }
  }

  #[test]
  fn moderation_body_is_content_moderated_regardless_of_4xx_code() {
    for status in [400, 403, 422] {
      let body = r#"{"detail":"Prompt was flagged by content moderation"}"#;
      let err = classify_higgsfield_http_error(status, Some(body)).unwrap_err();
      assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::ContentModerated { .. })), "status {status}");
    }
  }

  #[test]
  fn insufficient_credits_body_on_400() {
    let body = r#"{"detail":"Not enough credits to run this generation"}"#;
    let err = classify_higgsfield_http_error(400, Some(body)).unwrap_err();
    assert!(matches!(err, HiggsfieldError::Api(HiggsfieldApiError::InsufficientCredits { .. })));
  }

  #[test]
  fn clerk_error_envelope_is_extracted() {
    let body = r#"{"errors":[{"message":"Signed out","long_message":"The session is signed out","code":"session_signed_out"}],"clerk_trace_id":"abc"}"#;
    assert_eq!(extract_error_message(body).as_deref(), Some("The session is signed out"));
  }

  #[test]
  fn nested_error_message_is_extracted() {
    assert_eq!(extract_error_message(r#"{"error":{"message":"boom"}}"#).as_deref(), Some("boom"));
    assert_eq!(extract_error_message(r#"{"message":"plain"}"#).as_deref(), Some("plain"));
    assert_eq!(extract_error_message("[]"), None);
  }
}
