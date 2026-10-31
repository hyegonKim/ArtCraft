use tokens::tokens::media_files::MediaFileToken;
use tokens::tokens::model_weights::ModelWeightToken;

/// Video sources can be one of several:
///  - F: media_files (todo)
///  - U: media_uploads (legacy)

/// For image to image we probably want images
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StableDiffusionSource {
    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media File Token (media_files table)
    /// Serde cannot yet rename enum variants.
    F(String),

    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media Upload Token (media_uploads table)
    /// Serde cannot yet rename enum variants.
    U(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StableDiffusionArgs {

    #[serde(rename = "sd")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_sd_model_token: Option<ModelWeightToken>,

    #[serde(rename = "lm")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_lora_model_token: Option<ModelWeightToken>,

    #[serde(rename = "w")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_width: Option<u32>,

    #[serde(rename = "h")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_height: Option<u32>,

    #[serde(rename = "s")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_sampler: Option<String>,

    #[serde(rename = "p")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_prompt: Option<String>,

    #[serde(rename = "np")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_n_prompt: Option<String>,

    #[serde(rename = "se")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_seed: Option<i64>,

    #[serde(rename = "mu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_upload_path: Option<String>,

    #[serde(rename = "cf")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_cfg_scale: Option<u32>,

    #[serde(rename = "lu")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_lora_upload_path: Option<String>,

    #[serde(rename = "sa")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_number_of_samples: Option<u32>,

    #[serde(rename = "bc")]
    pub maybe_batch_count: Option<u32>,

    #[serde(rename = "t")]
    pub type_of_inference: String,

    // loRA / checkpoint description
    #[serde(rename = "de")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_description: Option<String>,
    // loRA / checkpoint name and description
    #[serde(rename = "na")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_name: Option<String>,

    #[serde(rename = "ve")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_version: Option<u32>
}


