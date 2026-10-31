use crate::state::app_config::AppConfig;
use crate::state::app_dir::AppDataRoot;
use crate::stubs::model_cache::ModelCache;
use crate::stubs::prompt_cache::PromptCache;
use crate::utils::image::decode_base64_image::decode_base64_image;
use chrono::Local;
use crockford::crockford_entropy_lower;
use image::{ImageFormat, RgbImage};
use log::info;
use tauri::{AppHandle, State};

const FILENAME_LENGTH : usize = 10;

/// This handler takes an image (as a base64 encoded string) and saves it.
#[tauri::command]
pub async fn save_image(
  image: &str,
  app_data_root: State<'_, AppDataRoot>,
) -> Result<String, String> {
  info!("save_image endpoint called.");

  let image = decode_base64_image(image)
    .map_err(|err| format!("Couldn't hydrate image from base64: {}", err))?;

  let assets_dir = app_data_root.assets_dir();
  
  let date_save_dir = assets_dir.make_or_get_current_date_dir()
    .map_err(|err| format!("Couldn't create assets save directory: {}", err))?;

  let entropy = crockford_entropy_lower(FILENAME_LENGTH); // TODO: These are ugly. A monotonic counter would be better.
  let filename = format!("{}.png", entropy);
  let filename = date_save_dir.join(filename);
  
  image.save_with_format(filename,  ImageFormat::Png)
    .map_err(|err| format!("Couldn't save image: {}", err))?;

  Ok("".to_string())
}
