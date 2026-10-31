use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const NANO_BANANA_MULTI_FUNCTION_IMAGE_GEN_PATH: &str = "/v1/generate/image/multi_function/nano_banana";

#[derive(Serialize, Deserialize, Debug)]
pub struct NanoBananaMultiFunctionImageGenRequest {
  /// Idempotency token to prevent duplicate requests.
  pub uuid_idempotency_token: String,

  /// Text prompt to generate the image from.
  pub prompt: Option<String>,

  /// Image media tokens to include in the editing context.
  /// If present, we're doing image editing (image-to-image / image-editing)
  /// If absent, we're doing image generation (text-to-image)
  pub image_media_tokens: Option<Vec<MediaFileToken>>,

  /// Number of images to generate. Default is one.
  pub num_images: Option<NanoBananaMultiFunctionImageGenNumImages>,

  /// Aspect ratio of the images to generate. Default is "1:1"
  pub aspect_ratio: Option<NanoBananaMultiFunctionImageGenAspectRatio>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NanoBananaMultiFunctionImageGenNumImages {
  One, // Default
  Two,
  Three,
  Four,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NanoBananaMultiFunctionImageGenAspectRatio {
  // Auto (only for image editing)
  Auto, // Default for image editing.
  // Square
  OneByOne, // Default for text-to-image
  // Wide
  FiveByFour,
  FourByThree,
  ThreeByTwo,
  SixteenByNine,
  TwentyOneByNine,
  // Tall
  FourByFive,
  ThreeByFour,
  TwoByThree,
  NineBySixteen, // NB: No NineByTwentyOne ?
}


#[derive(Serialize, Deserialize)]
pub struct NanoBananaMultiFunctionImageGenResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
