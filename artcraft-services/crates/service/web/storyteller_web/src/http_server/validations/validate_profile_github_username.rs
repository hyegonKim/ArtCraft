use once_cell::sync::Lazy;
use regex::Regex;

pub fn validate_profile_github_username(username: &str) -> Result<(), String> {
  // TODO: https://github.com/shinnn/github-username-regex
  static GITHUB_USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9\-]{1,39}$").expect("should be valid regex")
  });

  if username.len() < 1 {
    return Err("github username is too short".to_string());
  }

  if username.len() > 39 {
    return Err("github username is too long".to_string());
  }

  if !GITHUB_USERNAME_REGEX.is_match(username) {
    return Err("github username invalid".to_string());
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use crate::http_server::validations::validate_profile_github_username::validate_profile_github_username;

  #[test]
  fn valid_cases() {
    assert!(validate_profile_github_username("echelon").is_ok());
    assert!(validate_profile_github_username("a-b-c-D-E-F-0-1-2").is_ok());
  }

  #[test]
  fn invalid_cases() {
    assert!(validate_profile_github_username("").is_err());
  }
}
