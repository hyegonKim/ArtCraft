use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const GEMINI_25_FLASH_EDIT_IMAGE_PATH: &str = "/v1/generate/image/edit/gemini_25_flash";

#[derive(Serialize, Deserialize)]
pub struct Gemini25FlashEditImageRequest {
  /// Idempotency token to prevent duplicate requests.
  pub uuid_idempotency_token: String,

  /// Text prompt to generate the image from.
  pub prompt: Option<String>,

  // TODO: OneOf for image_urls or image_media_tokens
  /// Image media tokens to include in the editing context.
  pub image_media_tokens: Option<Vec<MediaFileToken>>,

  /// Number of images to generate. Default is one.
  pub num_images: Option<Gemini25FlashEditImageNumImages>,

  /// Quality of the image to generate. Default is High.
  pub image_quality: Option<Gemini25FlashEditImageImageQuality>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gemini25FlashEditImageNumImages {
  One, // Default
  Two,
  Three,
  Four,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gemini25FlashEditImageImageQuality {
  Auto,
  Low,
  Medium,
  High, // Default
}


#[derive(Serialize, Deserialize)]
pub struct Gemini25FlashEditImageResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
