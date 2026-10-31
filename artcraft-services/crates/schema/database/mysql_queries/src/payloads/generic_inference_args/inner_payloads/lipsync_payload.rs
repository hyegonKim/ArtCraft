//! Arguments for lipsync inferences

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LipsyncArgs {
  #[serde(rename = "a")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_audio_source: Option<LipsyncAnimationAudioSource>,

  #[serde(rename = "i")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_image_source: Option<LipsyncAnimationImageSource>,

  /// SadTalker --enhancer
  /// Which face enhancement algorithm to run.
  /// This makes the video larger in dimensions and higher quality.
  #[serde(rename = "f")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_face_enhancer: Option<FaceEnhancer>,

  /// SadTalker --pose_style
  /// Number between [0, 46) to control pose.
  #[serde(rename = "y")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_pose_style: Option<u8>,

  /// SadTalker --preprocess
  #[serde(rename = "p")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_preprocess: Option<Preprocess>,

  /// SadTalker --still
  #[serde(rename = "s")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_make_still: Option<bool>,

  /// Omit adding the watermark
  #[serde(rename = "m")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_remove_watermark: Option<bool>,

  /// Resize width
  /// NB: FOR TESTING ONLY
  #[serde(rename = "w")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_resize_width: Option<u32>,

  /// Resize height
  /// NB: FOR TESTING ONLY
  #[serde(rename = "h")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_resize_height: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Preprocess {
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "crop"
  #[serde(rename = "C")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  C,
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "extcrop"
  #[serde(rename = "EC")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  EC,
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "full"
  #[serde(rename = "F")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  F,
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "extfull"
  #[serde(rename = "EF")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  EF,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FaceEnhancer {
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "gfpgan"
  G,
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// "RestoreFormer"
  R,
}

/// Audio sources can be one of several:
///  - F: media_files (todo)
///  - U: media_uploads (legacy)
///  - T: tts_results (legacy)
///  - V: voice_conversion_results (legacy)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LipsyncAnimationAudioSource {
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// Media File Token (media_files table)
  /// Serde cannot yet rename enum variants.
  F(String),

  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// Media Upload Token (media_uploads table)
  /// Serde cannot yet rename enum variants.
  U(String),

  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// TTS Result Token (tts_results table)
  /// Serde cannot yet rename enum variants.
  T(String),

  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// Voice Conversion Result Token (voice_conversion_results table)
  /// Serde cannot yet rename enum variants.
  V(String),
}

// TODO: Hydrate to a public enum with easier to read names.
impl LipsyncAnimationAudioSource {
  pub fn media_file_token(token: &str) -> Self {
    LipsyncAnimationAudioSource::F(token.to_string())
  }
  pub fn media_upload_token(token: &str) -> Self {
    LipsyncAnimationAudioSource::U(token.to_string())
  }
  pub fn tts_result_token(token: &str) -> Self {
    LipsyncAnimationAudioSource::T(token.to_string())
  }
  pub fn voice_conversion_result_token(token: &str) -> Self {
    LipsyncAnimationAudioSource::V(token.to_string())
  }
}

/// Video sources can be one of several:
///  - F: media_files (todo)
///  - U: media_uploads (legacy)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LipsyncAnimationImageSource {
  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// Media File Token (media_files table)
  /// Serde cannot yet rename enum variants.
  F(String),

  // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  /// Media Upload Token (media_uploads table)
  /// Serde cannot yet rename enum variants.
  U(String),
}

impl LipsyncAnimationImageSource {
  pub fn media_file_token(token: &str) -> Self {
    LipsyncAnimationImageSource::F(token.to_string())
  }
  pub fn media_upload_token(token: &str) -> Self {
    LipsyncAnimationImageSource::U(token.to_string())
  }
}

#[cfg(test)]
mod tests {
  use crate::payloads::generic_inference_args::inner_payloads::lipsync_payload::{FaceEnhancer, LipsyncAnimationAudioSource, LipsyncAnimationImageSource, LipsyncArgs};

  fn assert_json_deserializes_to_match(json: &str, original: &LipsyncArgs) {
    let duplicate : LipsyncArgs = serde_json::de::from_str(json).unwrap();
    assert_eq!(&duplicate, original);
  }

  #[test]
  fn test_default() {
    let args = LipsyncArgs::default();
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_media_file() {
    let args = LipsyncArgs {
      maybe_audio_source: Some(LipsyncAnimationAudioSource::media_file_token("audio_media_file")),
      maybe_image_source: Some(LipsyncAnimationImageSource::media_file_token("image_media_file")),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"a":{"F":"audio_media_file"},"i":{"F":"image_media_file"}}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_media_upload() {
    let args = LipsyncArgs {
      maybe_audio_source: Some(LipsyncAnimationAudioSource::media_upload_token("audio_media_upload")),
      maybe_image_source: Some(LipsyncAnimationImageSource::media_upload_token("image_media_upload")),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"a":{"U":"audio_media_upload"},"i":{"U":"image_media_upload"}}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_tts_result() {
    let args = LipsyncArgs {
      maybe_audio_source: Some(LipsyncAnimationAudioSource::tts_result_token("audio_tts_result")),
      maybe_image_source: Some(LipsyncAnimationImageSource::media_upload_token("image_media_upload")),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"a":{"T":"audio_tts_result"},"i":{"U":"image_media_upload"}}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_voice_conversion_result() {
    let args = LipsyncArgs {
      maybe_audio_source: Some(LipsyncAnimationAudioSource::voice_conversion_result_token("audio_voice_conversion_result")),
      maybe_image_source: Some(LipsyncAnimationImageSource::media_upload_token("image_media_upload")),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"a":{"V":"audio_voice_conversion_result"},"i":{"U":"image_media_upload"}}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_face_enhancer_1() {
    let args = LipsyncArgs {
      maybe_face_enhancer: Some(FaceEnhancer::G),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"f":"G"}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_face_enhancer_2() {
    let args = LipsyncArgs {
      maybe_face_enhancer: Some(FaceEnhancer::R),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"f":"R"}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_width_and_height() {
    let args = LipsyncArgs {
      maybe_resize_width: Some(123),
      maybe_resize_height: Some(321),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"w":123,"h":321}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_maybe_make_still() {
    let args = LipsyncArgs {
      maybe_make_still: Some(true),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"s":true}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }

  #[test]
  fn test_maybe_remove_watermark() {
    let args = LipsyncArgs {
      maybe_remove_watermark: Some(true),
      ..Default::default()
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"m":true}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }
}
