use once_cell::sync::Lazy;
use regex::Regex;

static EXPECTED_FORMAT : Lazy<Regex> = Lazy::new(|| {
  // Support UUID with or without the dashes.
  Regex::new(r"^(([a-zA-Z0-9]{32})|([a-zA-Z0-9\-]{36}))$").expect("should be valid regex")
});

/// Validate the idempotency token
/// Idempotency tokens are supposed to be random entropy from the frontend to prevent duplicate
/// creation/insertion requests from being sent and acknowledged by the database.
/// We expect them to be in UUID format.
pub fn validate_idempotency_token_format(token: &str) -> Result<(), String> {
  if !EXPECTED_FORMAT.is_match(token) {
    return Err("idempotency token does not match the expected format".to_string());
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use crate::http_server::validations::validate_idempotency_token_format::validate_idempotency_token_format;

  #[test]
  fn valid_idempotency_tokens() {
    // UUID with the dashes
    assert!(validate_idempotency_token_format("bf12126a-2de0-4527-bbbf-156c4c45ae21").is_ok());
    assert!(validate_idempotency_token_format("220ace1b-db32-42c4-bb87-49176c85a907").is_ok());
    assert!(validate_idempotency_token_format("e0704f17-ee75-4f7c-ae4c-a1fda56a50cd").is_ok());

    // UUID without the dashes
    assert!(validate_idempotency_token_format("220ace1bdb3242c4bb8749176c85a907").is_ok());
  }

  #[test]
  fn invalid_idempotency_tokens() {
    assert!(!validate_idempotency_token_format("").is_ok());
    assert!(!validate_idempotency_token_format("    ").is_ok());
    assert!(!validate_idempotency_token_format("\n\n\n\n").is_ok());
    assert!(!validate_idempotency_token_format("bob").is_ok());
    assert!(!validate_idempotency_token_format("220ace1bdb324").is_ok());
    assert!(!validate_idempotency_token_format("220ace1bdb3242c4bb8749176c85a907ace0b0def0ff0").is_ok());
  }
}
