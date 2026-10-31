use artcraft_api_defs::common::responses::job_details::JobDetailsLivePortraitRequest;
use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;

pub fn extract_live_portrait_details(args: &PolymorphicInferenceArgs) -> Option<JobDetailsLivePortraitRequest> {
  let lp =
      if let PolymorphicInferenceArgs::Lp(live_portrait_args) = args {
        live_portrait_args
      } else {
        return None;
      };

  lp.portrait_media_file_token
      .as_ref()
      .zip(lp.driver_media_file_token.as_ref())
      .map(|(portrait, driver) | {
        JobDetailsLivePortraitRequest {
          source_media_file_token: portrait.clone(),
          face_driver_media_file_token: driver.clone(),
        }
      })
}

#[cfg(test)]
mod tests {
  use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;
  use mysql_queries::payloads::generic_inference_args::inner_payloads::live_portrait_payload::LivePortraitPayload;
  use tokens::tokens::media_files::MediaFileToken;

  use super::extract_live_portrait_details;

  #[test]
  fn test_valid_args_with_tokens() {
    let polymorphic_args = PolymorphicInferenceArgs::Lp(LivePortraitPayload {
      portrait_media_file_token: Some(MediaFileToken::new_from_str("portrait")),
      driver_media_file_token: Some(MediaFileToken::new_from_str("driver")),
      crop: None,
      remove_watermark: None,
      watermark_type: None,
      used_webcam: None,
      sleep_millis: None,
    });

    let details = extract_live_portrait_details(&polymorphic_args)
        .expect("should contain tokens");

    assert_eq!(details.source_media_file_token.as_str(), "portrait");
    assert_eq!(details.face_driver_media_file_token.as_str(), "driver");
  }
}
