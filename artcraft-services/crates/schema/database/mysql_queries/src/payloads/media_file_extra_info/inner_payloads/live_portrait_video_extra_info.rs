use tokens::tokens::media_files::MediaFileToken;

/// For things that don't fit in the schema
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivePortraitVideoExtraInfo {
  /// The portrait image or video (main source)
  #[serde(rename = "p")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_portrait_media_token: Option<MediaFileToken>,

  /// The driver video (face animation driver)
  #[serde(rename = "d")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to be fewer bytes.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_driver_video_media_token: Option<MediaFileToken>,
}

#[cfg(test)]
mod tests {
  use tokens::tokens::media_files::MediaFileToken;

  use crate::payloads::media_file_extra_info::inner_payloads::live_portrait_video_extra_info::LivePortraitVideoExtraInfo;
  use crate::payloads::media_file_extra_info::media_file_extra_info::MediaFileExtraInfo;

  #[test]
  fn base_case() {
    let payload = MediaFileExtraInfo::L(LivePortraitVideoExtraInfo {
      maybe_portrait_media_token: Some(MediaFileToken::new_from_str("portrait")),
      maybe_driver_video_media_token: Some(MediaFileToken::new_from_str("driver")),
    });

    let json = r#"{"L":{"p":"portrait","d":"driver"}}"#;

    let serialized = payload.to_json_string().unwrap();
    assert_eq!(&serialized, json);

    let deserialized = MediaFileExtraInfo::from_json_str(&json).unwrap();
    assert_eq!(deserialized, payload);
  }
}
