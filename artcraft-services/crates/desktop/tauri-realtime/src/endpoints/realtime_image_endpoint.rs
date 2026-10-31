#[cfg(not(target_os = "macos"))]
use ml_models::ml::stable_diffusion::lcm_pipeline::{lcm_pipeline, Args};

use crate::state::app_config::AppConfig;
use crate::state::app_dir::AppDataRoot;
use crate::stubs::model_cache::ModelCache;
use crate::stubs::prompt_cache::PromptCache;
use crate::utils::image::decode_base64_image::decode_base64_image;
use crate::utils::image::encode_dynamic_image_base64_png::encode_dynamic_image_base64_png;
use crate::utils::image::encode_rgb_image_base64_png::encode_rgb_image_base64_png;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};
use log::{error, info};
use ml_weights_registry::weights_registry::weights::{CLIP_JSON, LYKON_DEAMSHAPER_7_TEXT_ENCODER_FP16, LYKON_DEAMSHAPER_7_VAE, SDXL_TURBO_CLIP_TEXT_ENCODER, SIMIANLUO_LCM_DREAMSHAPER_V7_UNET};
use std::path::PathBuf;
use tauri::{AppHandle, State};


const PROMPT_FILENAME : &str = "prompt.txt";
const PROMPT: &str = "shiba inu, cute dog, detailed, walking in a wooded forest, photorealistic, 8k";

const NEGATIVE_PROMPT: &str = "bad quality, bad faces, poor quality, blurry faces, watermark";

const RANDOM_SEED: u32 = 42;


/// This handler takes an image (as a base64 encoded string) and a prompt and returns
/// an image (as a base64-encoded string).
#[tauri::command]
pub async fn infer_image(
  image: &str,
  prompt: Option<String>,
  strength: Option<f64>,
  seed: Option<u64>,
  model_config: State<'_, AppConfig>,
  model_cache: State<'_, ModelCache>,
  prompt_cache: State<'_, PromptCache>,
  app_data_root: State<'_, AppDataRoot>,
  app: AppHandle,
) -> Result<String, String> {
  info!("infer_image endpoint called.");

  let prompt = get_prompt_or_fallback(prompt);

  info!("Strength: {:?}; Prompt: {}", strength, prompt);

  let image = decode_base64_image(image)
    .map_err(|err| format!("Couldn't hydrate image from base64: {}", err))?;
  
  // TODO(bt,2025-02-17): Running out of vram with full image buffer size
  let mut image = image.resize(512, 512, FilterType::CatmullRom);

  let seed = seed.or(model_config.seed);

  #[cfg(not(target_os = "macos"))]
  {
    let result = infer_image_impl(
      &prompt,
      image,
      strength,
      seed,
      &model_config,
      &model_cache,
      prompt_cache,
      &app_data_root
    ).await;

    image = match result {
      Ok(img) => DynamicImage::ImageRgb8(img),
      Err(err) => {
        error!("There was an error generating the image: {:?}", err);
        return Err(format!("There was an error generating the image: {}", err));
      }
    };
  }

  let bytes = encode_dynamic_image_base64_png(image)
    .map_err(|err| format!("failure to encode image: {:?}", err))?;

  info!("Inference successful; image converted to base64, serving back to browser...");

  Ok(bytes)
}

#[cfg(not(target_os = "macos"))]
async fn infer_image_impl(
  prompt: &str,
  image: DynamicImage,
  strength: Option<f64>,
  seed: Option<u64>,
  config: &AppConfig,
  model_cache: &ModelCache,
  prompt_cache: State<'_, PromptCache>,
  app_data_root: &AppDataRoot,
) -> Result<RgbImage, String> {
  
  let weights_dir = app_data_root.weights_dir();
  let vae_path = weights_dir.weight_path(&LYKON_DEAMSHAPER_7_VAE);
  let unet_path = weights_dir.weight_path(&SIMIANLUO_LCM_DREAMSHAPER_V7_UNET);
  let clip_json_path = weights_dir.weight_path(&CLIP_JSON);
  //let clip_weights_path= weights_dir.weight_path(&LYKON_DEAMSHAPER_7_TEXT_ENCODER_FP16);
  let clip_weights_path= weights_dir.weight_path(&SDXL_TURBO_CLIP_TEXT_ENCODER);

  let args = Args {
    image: &image,
    prompt: prompt.to_string(),
    uncond_prompt: NEGATIVE_PROMPT.to_string(),
    model_cache,
    prompt_cache: &prompt_cache,
    img2img_strength: strength,
    cfg_scale: config.cfg_scale,
    use_flash_attn: true,
    model_config: &config.model_config,
    maybe_seed: seed,
    scheduler_steps: config.scheduler_steps,
    vae_path: &vae_path,
    unet_path: &unet_path,
    clip_json_path: &clip_json_path,
    clip_weights_path: &clip_weights_path,
  };

  let image = lcm_pipeline(args)
    .map_err(|err| format!("failure to encode image: {:?}", err))?;

  Ok(image)
}

fn get_prompt_or_fallback(user_prompt: Option<String>) -> String {
  let user_prompt = user_prompt.map(|prompt| prompt.trim().to_string())
    .filter(|prompt| !prompt.is_empty());

  if let Some(prompt) = user_prompt {
    return prompt;
  }

  let prompt_file = PathBuf::from(PROMPT_FILENAME)
    .canonicalize()
    .unwrap_or_else(|_| PathBuf::from(PROMPT_FILENAME));

  std::fs::read_to_string(&prompt_file)
    .map_err(|err| format!("Failed to read prompt file: {}", err))
    .unwrap_or_else(|_| {
      error!("Failed to read prompt file: {:?}", prompt_file);
      PROMPT.to_string()
    })
    .trim()
    .to_string()
}
