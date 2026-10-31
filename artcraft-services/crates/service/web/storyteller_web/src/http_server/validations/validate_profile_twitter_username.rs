use once_cell::sync::Lazy;
use regex::Regex;

pub fn validate_profile_twitter_username(username: &str) -> Result<(), String> {
  static TWITTER_USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^@?[A-Za-z0-9_]{4,15}$").expect("should be valid regex")
  });

  if username.starts_with("@") {
    if username.len() < 5 {
      return Err("twitter username is too short".to_string());
    }
    if username.len() > 16 {
      return Err("twitter username is too long".to_string());
    }
  } else {
    if username.len() < 4 {
      return Err("twitter username is too short".to_string());
    }
    if username.len() > 15 {
      return Err("twitter username is too long".to_string());
    }
  }

  if !TWITTER_USERNAME_REGEX.is_match(username) {
    return Err("twitter username invalid".to_string());
  }

  Ok(())
}

/// Remove the leading '@' for consistency and better internal use.
pub fn normalize_twitter_username_for_storage(username: &str) -> String {
  username.replace("@", "")
}

#[cfg(test)]
mod tests {
  use crate::http_server::validations::validate_profile_twitter_username::{normalize_twitter_username_for_storage, validate_profile_twitter_username};

  #[test]
  fn valid_cases() {
    assert!(validate_profile_twitter_username("@echelon").is_ok());
    assert!(validate_profile_twitter_username("echelon").is_ok());
    assert!(validate_profile_twitter_username("four").is_ok());
    assert!(validate_profile_twitter_username("@four").is_ok());
    assert!(validate_profile_twitter_username("123456789012345").is_ok());
    assert!(validate_profile_twitter_username("@123456789012345").is_ok());
    assert!(validate_profile_twitter_username("a_A_b_B_c_C_d_D").is_ok());
    assert!(validate_profile_twitter_username("@a_A_b_B_c_C_d_D").is_ok());
  }

  #[test]
  fn invalid_cases() {
    assert!(validate_profile_twitter_username("").is_err());
    assert!(validate_profile_twitter_username("@").is_err());
    assert!(validate_profile_twitter_username("    ").is_err());
    assert!(validate_profile_twitter_username("!!!!!!!!!!!").is_err());
    assert!(validate_profile_twitter_username("a").is_err());
    assert!(validate_profile_twitter_username("@a").is_err());
    assert!(validate_profile_twitter_username("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_err());
  }

  #[test]
  fn test_normalize_twitter_username_for_storage() {
    assert_eq!(normalize_twitter_username_for_storage("echelon"), "echelon".to_string());
    assert_eq!(normalize_twitter_username_for_storage("@echelon"), "echelon".to_string());
  }
}