use serde_derive::{Deserialize, Serialize};

use crate::enums::common::generation::common_model_type::CommonModelType;
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

/// Request body for creating a character.
#[derive(Deserialize)]
pub struct CreateCharacterRequest {
  /// Idempotency token to prevent duplicate requests.
  pub uuid_idempotency_token: String,

  /// The model to create the character for.
  pub model: CommonModelType,

  /// The reference image media file token.
  pub image_media_token: MediaFileToken,

  /// Name of the character.
  pub character_name: String,

  /// Description of the character.
  pub character_description: Option<String>,
}

/// Response body for creating a character.
#[derive(Serialize)]
pub struct CreateCharacterResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
