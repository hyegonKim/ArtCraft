use once_cell::sync::Lazy;
use regex::Regex;

pub fn validate_profile_discord_username(username: &str) -> Result<(), String> {
  static DISCORD_USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^@?[^#@:]{2,32}(#[0-9]{4})?$").expect("should be valid regex")
  });

  if username.len() < 2 {
    return Err("discord username is too short".to_string());
  }

  if username.len() > 40 {
    return Err("discord username is too long".to_string());
  }

  if !DISCORD_USERNAME_REGEX.is_match(username) {
    return Err("discord username invalid".to_string());
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use crate::http_server::validations::validate_profile_discord_username::validate_profile_discord_username;

  #[test]
  fn valid_cases() {
    assert!(validate_profile_discord_username("echelon#0001").is_ok());
    assert!(validate_profile_discord_username("@echelon#0001").is_ok());
    assert!(validate_profile_discord_username("echelon").is_ok());
    assert!(validate_profile_discord_username("@echelon").is_ok());
  }

  #[test]
  fn invalid_cases() {
    assert!(validate_profile_discord_username("").is_err());
  }
}