use tokens::tokens::media_files::MediaFileToken;
use crate::payloads::generic_inference_args::common::watermark_type::WatermarkType;

// **DO NOT CHANGE THE NAMES OF FIELDS WITHOUT A MIGRATION STRATEGY**
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LivePortraitPayload {
  /// Either an image or video.
  #[serde(rename = "p")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub portrait_media_file_token: Option<MediaFileToken>,

  /// A video that drives the face animation.
  #[serde(rename = "d")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub driver_media_file_token: Option<MediaFileToken>,

  #[serde(rename = "c")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub crop: Option<CropDimensions>,

  #[serde(rename = "rm")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub remove_watermark: Option<bool>,

  #[serde(rename = "w")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub watermark_type: Option<WatermarkType>,

  /// Flag to indicate the webcam mode was used
  #[serde(rename = "wc")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub used_webcam: Option<bool>,

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
