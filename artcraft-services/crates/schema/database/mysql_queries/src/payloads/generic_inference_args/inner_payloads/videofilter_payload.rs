use tokens::tokens::model_weights::ModelWeightToken;

/// Video sources can be one of several:
///  - F: media_files (todo)
///  - U: media_uploads (legacy)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VideofilterVideoSource {
    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media File Token (media_files table)
    /// Serde cannot yet rename enum variants.
    F(String),

    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media Upload Token (media_uploads table)
    /// Serde cannot yet rename enum variants.
    U(String),
}


impl VideofilterVideoSource {
    pub fn media_file_token(token: &str) -> Self {
        VideofilterVideoSource::F(token.to_string())
    }
    pub fn media_upload_token(token: &str) -> Self {
        VideofilterVideoSource::U(token.to_string())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RerenderArgs {
    #[serde(rename = "vs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_video_source: Option<VideofilterVideoSource>,

    #[serde(rename = "sd")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_sd_model_token: Option<ModelWeightToken>,

    #[serde(rename = "lora")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_lora_model_token: Option<ModelWeightToken>,

    #[serde(rename = "p")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_prompt: Option<String>,

    #[serde(rename = "ap")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_a_prompt: Option<String>,

    #[serde(rename = "np")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_n_prompt: Option<String>,

    #[serde(rename = "se")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_seed: Option<i32>,
}
