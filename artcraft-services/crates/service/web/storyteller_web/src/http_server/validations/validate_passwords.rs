
pub fn validate_passwords(password: &str, password_confirmation: &str) -> Result<(), String> {
  if password.len() < 6 {
    return Err("password is too short".to_string());
  }

  if password != password_confirmation {
    return Err("passwords do not match".to_string());
  }

  Ok(())
}
