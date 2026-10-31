use once_cell::sync::Lazy;
use regex::Regex;

pub fn validate_profile_cashapp_username(username: &str) -> Result<(), String> {
  static CASHAPP_USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\$?.{1,20}$").expect("should be valid regex")
  });

  if username.starts_with("$") {
    if username.len() < 2 {
      return Err("cashapp username is too short".to_string());
    }
    if username.len() > 21 {
      return Err("cashapp username is too long".to_string());
    }
  } else {
    if username.len() < 1 {
      return Err("cashapp username is too short".to_string());
    }
    if username.len() > 20 {
      return Err("cashapp username is too long".to_string());
    }
  }

  if !CASHAPP_USERNAME_REGEX.is_match(username) {
    return Err("cashapp username invalid".to_string());
  }

  Ok(())
}

/// Remove the leading '$' for consistency and better internal use.
pub fn normalize_cashapp_username_for_storage(username: &str) -> String {
  username.replace("$", "")
}

#[cfg(test)]
mod tests {
  use crate::http_server::validations::validate_profile_cashapp_username::{normalize_cashapp_username_for_storage, validate_profile_cashapp_username};

  #[test]
  fn valid_cases() {
    assert!(validate_profile_cashapp_username("echelon").is_ok());
    assert!(validate_profile_cashapp_username("$echelon").is_ok());
    assert!(validate_profile_cashapp_username("a").is_ok());
    assert!(validate_profile_cashapp_username("12345678901234567890").is_ok());
    assert!(validate_profile_cashapp_username("$12345678901234567890").is_ok());
  }

  #[test]
  fn invalid_cases() {
    assert!(validate_profile_cashapp_username("").is_err());
    assert!(validate_profile_cashapp_username("$").is_err());
    assert!(validate_profile_cashapp_username("123456789012345678901").is_err());
  }

  #[test]
  fn test_normalize_twitter_username_for_storage() {
    assert_eq!(normalize_cashapp_username_for_storage("echelon"), "echelon".to_string());
    assert_eq!(normalize_cashapp_username_for_storage("$echelon"), "echelon".to_string());
  }
}