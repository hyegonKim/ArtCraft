
/// Basic email format validation
pub fn validate_email_address_format(email: &str) -> Result<(), String> {
  let email = email.trim();

  if email.is_empty() {
    return Err("email address cannot be empty".to_string());
  }

  if email.len() > 255 {
    return Err("email address is too long".to_string());
  }

  // Basic format check: must contain exactly one @ with content on both sides
  let at_count = email.matches('@').count();
  if at_count != 1 {
    return Err("email address must contain exactly one @".to_string());
  }

  let parts: Vec<&str> = email.split('@').collect();
  if parts.len() != 2 {
    return Err("invalid email format".to_string());
  }

  let local = parts[0];
  let domain = parts[1];

  if local.is_empty() {
    return Err("email address must have content before @".to_string());
  }

  if domain.is_empty() {
    return Err("email address must have a domain after @".to_string());
  }

  // Domain must contain at least one dot
  if !domain.contains('.') {
    return Err("email domain must contain a dot".to_string());
  }

  // Domain cannot start or end with a dot
  if domain.starts_with('.') || domain.ends_with('.') {
    return Err("email domain cannot start or end with a dot".to_string());
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::validate_email_address_format;

  // Valid emails
  #[test]
  fn test_valid_simple_email() {
    assert!(validate_email_address_format("user@example.com").is_ok());
  }

  #[test]
  fn test_valid_email_with_subdomain() {
    assert!(validate_email_address_format("user@mail.example.com").is_ok());
  }

  #[test]
  fn test_valid_email_with_plus() {
    assert!(validate_email_address_format("user+tag@example.com").is_ok());
  }

  #[test]
  fn test_valid_email_with_dots_in_local() {
    assert!(validate_email_address_format("first.last@example.com").is_ok());
  }

  #[test]
  fn test_valid_email_with_numbers() {
    assert!(validate_email_address_format("user123@example123.com").is_ok());
  }

  #[test]
  fn test_valid_email_trimmed() {
    assert!(validate_email_address_format("  user@example.com  ").is_ok());
  }

  // Empty/whitespace
  #[test]
  fn test_invalid_empty() {
    let result = validate_email_address_format("");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));
  }

  #[test]
  fn test_invalid_whitespace_only() {
    let result = validate_email_address_format("   ");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));
  }

  // Length
  #[test]
  fn test_invalid_too_long() {
    let long_email = format!("{}@example.com", "a".repeat(250));
    let result = validate_email_address_format(&long_email);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too long"));
  }

  #[test]
  fn test_valid_at_max_length() {
    // 255 chars is the max
    let local_part = "a".repeat(242); // 242 + @ + example.com (11) = 254
    let email = format!("{}@example.com", local_part);
    assert_eq!(email.len(), 254);
    assert!(validate_email_address_format(&email).is_ok());
  }

  // @ symbol
  #[test]
  fn test_invalid_no_at() {
    let result = validate_email_address_format("userexample.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exactly one @"));
  }

  #[test]
  fn test_invalid_multiple_at() {
    let result = validate_email_address_format("user@@example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exactly one @"));
  }

  #[test]
  fn test_invalid_multiple_at_separated() {
    let result = validate_email_address_format("user@name@example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exactly one @"));
  }

  // Local part
  #[test]
  fn test_invalid_no_local_part() {
    let result = validate_email_address_format("@example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("content before @"));
  }

  // Domain part
  #[test]
  fn test_invalid_no_domain() {
    let result = validate_email_address_format("user@");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("domain after @"));
  }

  #[test]
  fn test_invalid_domain_no_dot() {
    let result = validate_email_address_format("user@localhost");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("must contain a dot"));
  }

  #[test]
  fn test_invalid_domain_starts_with_dot() {
    let result = validate_email_address_format("user@.example.com");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot start or end with a dot"));
  }

  #[test]
  fn test_invalid_domain_ends_with_dot() {
    let result = validate_email_address_format("user@example.com.");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot start or end with a dot"));
  }

  #[test]
  fn test_invalid_domain_only_dot() {
    let result = validate_email_address_format("user@.");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot start or end with a dot"));
  }
}