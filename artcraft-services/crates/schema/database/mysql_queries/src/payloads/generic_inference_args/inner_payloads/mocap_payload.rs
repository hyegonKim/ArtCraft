/// Video sources can be one of several:
///  - F: media_files (todo)
///  - U: media_uploads (legacy)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MocapVideoSource {
    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media File Token (media_files table)
    /// Serde cannot yet rename enum variants.
    F(String),

    // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
    /// Media Upload Token (media_uploads table)
    /// Serde cannot yet rename enum variants.
    U(String),
}


impl MocapVideoSource {
    pub fn media_file_token(token: &str) -> Self {
        MocapVideoSource::F(token.to_string())
    }
    pub fn media_upload_token(token: &str) -> Self {
        MocapVideoSource::U(token.to_string())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MocapArgs {
    #[serde(rename = "vs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_video_source: Option<MocapVideoSource>,

    #[serde(rename = "ik1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_ik1: Option<f32>,

    #[serde(rename = "ik2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_ik2: Option<i32>,

    #[serde(rename = "ik3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_ik3: Option<i32>,

    #[serde(rename = "sm1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_smoothing1: Option<f32>,

    #[serde(rename = "sm2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_smoothing2: Option<f32>,

    #[serde(rename = "si1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_size1: Option<i32>,

    #[serde(rename = "si2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_size2: Option<i32>,
}
