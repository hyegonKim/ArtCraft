use serde_derive::{Deserialize, Serialize};
use utoipa::ToSchema;

use enums::by_table::media_files::media_file_origin_category::MediaFileOriginCategory;
use enums::by_table::media_files::media_file_origin_model_type::MediaFileOriginModelType;
use enums::by_table::media_files::media_file_origin_product_category::MediaFileOriginProductCategory;
use tokens::tokens::model_weights::ModelWeightToken;

/// Fields useful for enriching media file listings
#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct MediaFileOriginDetails {
  /// Where the file came from (broadly)
  pub origin_category: MediaFileOriginCategory,

  /// Where the file came from (specifically, a product area, eg. "face_animator")
  pub product_category: MediaFileOriginProductCategory,

  /// If the file was produced by a model, the details.
  pub maybe_model: Option<MediaFileModelDetails>,
}

/// Information about the model weights
/// https://serde.rs/enum-representations.html#untagged
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum MediaFileModelDetails {
  // Try the specific shape first so deserialization retains token and title.
  /// A model weight is typically a user-submitted model.
  /// We store lots of these.
  ModelWeight {
    /// The type of model weight
    model_type: MediaFileOriginModelType,
    /// The model token
    token: ModelWeightToken,
    /// The model title (typically only populated for `model_weights` models, not legacy tables such as `tts_models`.)
    title: String,
  },
  /// A system model is one that we maintain and update,
  /// eg. SadTalker, Wav2Lip, MocapNet, etc.
  SystemModel {
    /// The type of model weight
    model_type: MediaFileOriginModelType,
  },
}
