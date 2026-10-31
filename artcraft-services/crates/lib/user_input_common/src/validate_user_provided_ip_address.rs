use once_cell::sync::Lazy;
use regex::Regex;

static IP_ADDRESS_REGEX : Lazy<Regex> = Lazy::new(|| {
  // TODO: This doesn't require valid IP addresses. Just a number/dot format.
  Regex::new(r"^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$").expect("should be valid regex")
});

/// Do not use this for IP addresses from requests.
/// Only for moderator-input IP addresses
pub fn validate_user_provided_ip_address(ip_address: &str) -> Result<(), String> {
  if !IP_ADDRESS_REGEX.is_match(ip_address) {
    return Err("ip address is invalid".to_string());
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use crate::validate_user_provided_ip_address::validate_user_provided_ip_address;

  #[test]
  fn valid_cases() {
    assert!(validate_user_provided_ip_address("0.0.0.0").is_ok());
    assert!(validate_user_provided_ip_address("1.1.1.1").is_ok());
    assert!(validate_user_provided_ip_address("1.2.3.4").is_ok());
    assert!(validate_user_provided_ip_address("111.222.3.0").is_ok());
    assert!(validate_user_provided_ip_address("127.0.0.1").is_ok());
    assert!(validate_user_provided_ip_address("255.255.255.255").is_ok());
  }

  #[test]
  fn invalid_cases() {
    assert!(validate_user_provided_ip_address("    255.255.255.255   ").is_err());
    assert!(validate_user_provided_ip_address("").is_err());
    assert!(validate_user_provided_ip_address(".1.1.1.1").is_err());
    assert!(validate_user_provided_ip_address("1.1.1").is_err());
    assert!(validate_user_provided_ip_address("127.0.0.1notvalid").is_err());
    assert!(validate_user_provided_ip_address("127.127.127.127.").is_err());
  }
}
