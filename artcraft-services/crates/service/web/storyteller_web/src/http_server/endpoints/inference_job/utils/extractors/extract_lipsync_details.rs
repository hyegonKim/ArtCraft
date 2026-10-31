use artcraft_api_defs::common::responses::job_details::JobDetailsLipsyncRequest;
use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;
use mysql_queries::payloads::generic_inference_args::inner_payloads::face_fusion_payload::FaceFusionPayload;
use mysql_queries::payloads::generic_inference_args::inner_payloads::lipsync_payload::{LipsyncAnimationAudioSource, LipsyncAnimationImageSource, LipsyncArgs};
use tokens::tokens::media_files::MediaFileToken;

/// Extract args for SadTalker and FaceFusion jobs.
pub fn extract_lipsync_details(args: &PolymorphicInferenceArgs) -> Option<JobDetailsLipsyncRequest> {
  match args {
    // Face Fusion
    PolymorphicInferenceArgs::Ff(args) => extract_face_fusion(args),
    // Sad Talker
    PolymorphicInferenceArgs::La(args) => extract_sad_talker(args),
    // Non-matching job types
    _ => None,
  }
}

fn extract_face_fusion(args: &FaceFusionPayload) -> Option<JobDetailsLipsyncRequest> {
  args.audio_media_file_token
      .as_ref()
      .zip(args.image_or_video_media_file_token.as_ref())
      .map(|(audio, image_or_video)| {
        JobDetailsLipsyncRequest {
          audio_source_token: audio.clone(),
          image_or_video_source_token: image_or_video.clone(),
        }
      })
}

fn extract_sad_talker(args: &LipsyncArgs) -> Option<JobDetailsLipsyncRequest> {
  args.maybe_audio_source
      .as_ref()
      .map(|polymorphic| match polymorphic {
        // NB: We have enums over *old* record types: TTS results, Media Uploads, etc.
        LipsyncAnimationAudioSource::F(token) => Some(MediaFileToken::new_from_str(token)),
        _ => None,
      })
      .flatten()
      .zip(args.maybe_image_source.as_ref()
          .map(|polymorphic| match polymorphic {
            // NB: We have enums over *old* record types: TTS results, Media Uploads, etc.
            LipsyncAnimationImageSource::F(token) => Some(MediaFileToken::new_from_str(token)),
            _ => None,
          }).flatten())
      .map(|(audio, image_or_video)| {
        JobDetailsLipsyncRequest {
          audio_source_token: audio,
          image_or_video_source_token: image_or_video,
        }
      })
}

#[cfg(test)]
mod tests {
  use mysql_queries::payloads::generic_inference_args::generic_inference_args::PolymorphicInferenceArgs;
  use mysql_queries::payloads::generic_inference_args::inner_payloads::face_fusion_payload::FaceFusionPayload;
  use tokens::tokens::media_files::MediaFileToken;

  use super::extract_lipsync_details;

  #[test]
  fn test_valid_args_with_tokens() {
    let polymorphic_args = PolymorphicInferenceArgs::Ff(FaceFusionPayload{
      audio_media_file_token: Some(MediaFileToken::new_from_str("audio")),
      image_or_video_media_file_token: Some(MediaFileToken::new_from_str("image")),
      crop: None,
      watermark_type: None,
      sleep_millis: None,
    });

    let details = extract_lipsync_details(&polymorphic_args)
        .expect("should contain tokens");

    assert_eq!(details.audio_source_token.as_str(), "audio");
    assert_eq!(details.image_or_video_source_token.as_str(), "image");
  }
}
