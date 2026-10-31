use crate::enums::common::generation::common_aspect_ratio::CommonAspectRatio;
use crate::enums::common::generation::common_generation_mode::CommonGenerationMode;
use crate::enums::common::generation::common_model_type::CommonModelType;
use crate::enums::common::generation::common_resolution::CommonResolution;
use core_types::enums::generation_source::GenerationSource;
use serde_derive::{Deserialize, Serialize};
use sqlite_identifiers::ids::prompt_token::PromptToken;

pub const CREATE_PROMPT_PATH: &str = "/v1/prompts/create";

#[derive(Serialize, Deserialize)]
pub struct CreatePromptRequest {
  /// Idempotency token to prevent duplicate requests.
  pub uuid_idempotency_token: String,

  /// OPTIONAL. The positive prompt.
  pub positive_prompt: Option<String>,

  /// OPTIONAL. The negative prompt.
  pub negative_prompt: Option<String>,

  /// OPTIONAL. The model type.
  pub model_type: Option<CommonModelType>,

  /// OPTIONAL. The service used.
  pub generation_provider: Option<GenerationSource>,

  /// OPTIONAL. The generation mode (eg. keyframe, reference, inpaint, etc.)
  pub maybe_generation_mode: Option<CommonGenerationMode>,

  /// OPTIONAL. The aspect ratio (eg. square, auto, wide_three_by_two, etc.)
  pub maybe_aspect_ratio: Option<CommonAspectRatio>,

  /// OPTIONAL. The resolution (eg. one_k, two_k, four_k, etc.)
  pub maybe_resolution: Option<CommonResolution>,

  /// OPTIONAL. The number of outputs to generate (0-255).
  pub maybe_batch_count: Option<u8>,

  /// OPTIONAL. Whether to generate audio.
  pub maybe_generate_audio: Option<bool>,

  /// OPTIONAL. The duration in seconds.
  pub maybe_duration_seconds: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct CreatePromptResponse {
  pub success: bool,
  pub prompt_token: PromptToken,
}
