use tokens::tokens::media_files::MediaFileToken;

use crate::payloads::generic_inference_args::common::watermark_type::WatermarkType;

// **DO NOT CHANGE THE NAMES OF FIELDS WITHOUT A MIGRATION STRATEGY**
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FaceFusionPayload {
  /// Audio media (eg. from tts, vc, or upload)
  #[serde(rename = "a")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub audio_media_file_token: Option<MediaFileToken>,

  /// Image or video media
  #[serde(rename = "i")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image_or_video_media_file_token: Option<MediaFileToken>,

  #[serde(rename = "c")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub crop: Option<CropDimensions>,

  #[serde(rename = "w")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub watermark_type: Option<WatermarkType>,

  /// This is a debugging flag.
  #[serde(rename = "sp")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sleep_millis: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CropDimensions {
  pub x: u32,
  pub y: u32,
  #[serde(rename = "h")]
  pub height: u32,
  #[serde(rename = "w")]
  pub width: u32,
}
