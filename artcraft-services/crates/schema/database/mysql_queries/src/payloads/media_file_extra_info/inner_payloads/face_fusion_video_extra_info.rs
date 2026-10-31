use tokens::tokens::media_files::MediaFileToken;

/// For things that don't fit in the schema
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FaceFusionVideoExtraInfo {
  /// The audio media token
  #[serde(rename = "a")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_audio_media_token: Option<MediaFileToken>,

  /// The image or video media token
  #[serde(rename = "i")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub image_or_video_media_token: Option<MediaFileToken>,
}

#[cfg(test)]
mod tests {
  use tokens::tokens::media_files::MediaFileToken;

  use crate::payloads::media_file_extra_info::inner_payloads::face_fusion_video_extra_info::FaceFusionVideoExtraInfo;
  use crate::payloads::media_file_extra_info::media_file_extra_info::MediaFileExtraInfo;

  #[test]
  fn base_case() {
    let payload = MediaFileExtraInfo::F(FaceFusionVideoExtraInfo {
      maybe_audio_media_token: Some(MediaFileToken::new_from_str("audio")),
      image_or_video_media_token: Some(MediaFileToken::new_from_str("image")),
    });

    let json = r#"{"F":{"a":"audio","i":"image"}}"#;

    let serialized = payload.to_json_string().unwrap();
    assert_eq!(&serialized, json);

    let deserialized = MediaFileExtraInfo::from_json_str(&json).unwrap();
    assert_eq!(deserialized, payload);
  }
}
