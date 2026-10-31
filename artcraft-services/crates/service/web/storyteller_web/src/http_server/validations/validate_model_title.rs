pub fn validate_model_title(title: &str) -> Result<(), String> {
  if title.len() < 3 {
    return Err("title is too short".to_string());
  }

  if title.len() > 200 {
    return Err("title is too long".to_string());
  }

  Ok(())
}
