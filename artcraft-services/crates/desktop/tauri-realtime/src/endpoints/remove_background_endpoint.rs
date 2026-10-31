//! Adapted from https://github.com/dnanhkhoa/rust-background-removal

#[cfg(not(target_os = "macos"))]
use ml_models::ml::background_removal::remove_image_background::remove_image_background;

use crate::state::app_dir::AppDataRoot;
use crate::utils::image::decode_base64_image::decode_base64_image;
use crate::utils::image::encode_dynamic_image_base64_png::encode_dynamic_image_base64_png;
use image::{imageops, DynamicImage};
use log::info;
use ml_weights_registry::weights_registry::weights::DIS_MEDIUM_ONNX;
use tauri::State;

/// This handler removes the background from an image.
#[tauri::command]
pub async fn remove_background(
  image: &str,
  app_data_root: State<'_, AppDataRoot>,
) -> Result<String, String> {
  info!("remove_background endpoint called.");

  let mut image = decode_base64_image(image)
      .map_err(|err| format!("Couldn't hydrate image from base64: {}", err))?;

  #[cfg(not(target_os = "macos"))]
  {
    image = remove_background_impl(&app_data_root, image).await?;
  }

  let image = encode_dynamic_image_base64_png(image)
    .map_err(|err| format!("failure to encode image: {:?}", err))?;

  Ok(image)
}

#[cfg(not(target_os = "macos"))]
async fn remove_background_impl(app_data_root: &AppDataRoot, image: DynamicImage) -> Result<DynamicImage, String> {
  let model_path = app_data_root.weights_dir().weight_path(&DIS_MEDIUM_ONNX);

  let image = remove_image_background(model_path, image)
      .await
      .map_err(|err| format!("Couldn't remove image from image: {}", err))?;

  Ok(image)
}
