use log::info;


#[tauri::command]
pub async fn inpaint_image(
  image: &str,
  mask: &str,
  prompt: String,
) -> Result<String, String> {
  info!("inpaint_endpoint endpoint called.");

  Ok(mask.to_string())
}
