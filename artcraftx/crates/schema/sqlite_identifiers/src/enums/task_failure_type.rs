use std::collections::BTreeSet;

use crate::enums::enum_error::EnumError;
#[cfg(test)]
use strum::EnumCount;
#[cfg(test)]
use strum::EnumIter;

/// Failure type for tasks in the Tauri desktop app.
///
/// Mirrors the relevant variants from `FrontendFailureCategory` so the desktop
/// client can display localized failure information without depending / breaking on the
/// server-side enum directly.
#[cfg_attr(test, derive(EnumIter, EnumCount))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFailureType {
  /// Catch-all for unknown failures.
  Unknown,

  RuleBansUserImage,
  RuleBansUserImageWithFaces,
  RuleBansUserTextPrompt,
  RuleBansUserContent,

  RuleBansGeneratedVideo,
  RuleBansGeneratedAudio,
  RuleBansGeneratedContent,

  /// No foreground subject could be detected in the input image after
  /// background removal (e.g. TripoSplat image-to-splat).
  NoForegroundSubjectDetected,

  /// The input file's format isn't supported by the model (e.g. Hunyuan 3D
  /// Part only accepts FBX input).
  FormatNotSupported,

  /// No reason given for generation failure, but this matches what we were told.
  GenerationFailed,

  /// When a face is not detected in the image used for animation.
  FaceNotDetected,

  /// The user stepped away and a keep-alive-required workload timed out.
  KeepAliveElapsed,

  /// A feature isn't complete somewhere in the code.
  NotYetImplemented,

  /// A transient error happened; the workload will be retried.
  RetryableWorkerError,

  /// Model content rules were violated.
  ModelRulesViolation,

  /// The uploaded file exceeds the maximum allowed size for this model.
  FilesizeTooLarge,

  /// The uploaded image dimensions are below the minimum required by the model.
  ImageDimensionsTooSmall,

  /// The uploaded image dimensions exceed the maximum allowed by the model.
  ImageDimensionsTooLarge,
}

impl_enum_display_and_debug_using_to_str!(TaskFailureType);

impl TaskFailureType {

  pub const fn to_str(&self) -> &'static str {
    match self {
      Self::Unknown => "unknown",
      Self::RuleBansUserImage => "rule_bans_user_image",
      Self::RuleBansUserImageWithFaces => "rule_bans_user_image_with_faces",
      Self::RuleBansUserTextPrompt => "rule_bans_user_text_prompt",
      Self::RuleBansUserContent => "rule_bans_user_content",
      Self::RuleBansGeneratedVideo => "rule_bans_generated_video",
      Self::RuleBansGeneratedAudio => "rule_bans_generated_audio",
      Self::RuleBansGeneratedContent => "rule_bans_generated_content",
      Self::NoForegroundSubjectDetected => "no_foreground_subject_detected",
      Self::FormatNotSupported => "format_not_supported",
      Self::GenerationFailed => "generation_failed",
      Self::FaceNotDetected => "face_not_detected",
      Self::KeepAliveElapsed => "keep_alive_elapsed",
      Self::NotYetImplemented => "not_yet_implemented",
      Self::RetryableWorkerError => "retryable_worker_error",
      Self::ModelRulesViolation => "model_rules_violation",
      Self::FilesizeTooLarge => "filesize_too_large",
      Self::ImageDimensionsTooSmall => "image_dimensions_too_small",
      Self::ImageDimensionsTooLarge => "image_dimensions_too_large",
    }
  }

  pub fn from_str(value: &str) -> Result<Self, EnumError> {
    match value {
      "unknown" => Ok(Self::Unknown),
      "rule_bans_user_image" => Ok(Self::RuleBansUserImage),
      "rule_bans_user_image_with_faces" => Ok(Self::RuleBansUserImageWithFaces),
      "rule_bans_user_text_prompt" => Ok(Self::RuleBansUserTextPrompt),
      "rule_bans_user_content" => Ok(Self::RuleBansUserContent),
      "rule_bans_generated_video" => Ok(Self::RuleBansGeneratedVideo),
      "rule_bans_generated_audio" => Ok(Self::RuleBansGeneratedAudio),
      "rule_bans_generated_content" => Ok(Self::RuleBansGeneratedContent),
      "no_foreground_subject_detected" => Ok(Self::NoForegroundSubjectDetected),
      "format_not_supported" => Ok(Self::FormatNotSupported),
      "generation_failed" => Ok(Self::GenerationFailed),
      "face_not_detected" => Ok(Self::FaceNotDetected),
      "keep_alive_elapsed" => Ok(Self::KeepAliveElapsed),
      "not_yet_implemented" => Ok(Self::NotYetImplemented),
      "retryable_worker_error" => Ok(Self::RetryableWorkerError),
      "model_rules_violation" => Ok(Self::ModelRulesViolation),
      "filesize_too_large" => Ok(Self::FilesizeTooLarge),
      "image_dimensions_too_small" => Ok(Self::ImageDimensionsTooSmall),
      "image_dimensions_too_large" => Ok(Self::ImageDimensionsTooLarge),
      _ => Err(EnumError::CouldNotConvertFromString(value.to_string())),
    }
  }

  pub fn all_variants() -> BTreeSet<Self> {
    BTreeSet::from([
      Self::Unknown,
      Self::RuleBansUserImage,
      Self::RuleBansUserImageWithFaces,
      Self::RuleBansUserTextPrompt,
      Self::RuleBansUserContent,
      Self::RuleBansGeneratedVideo,
      Self::RuleBansGeneratedAudio,
      Self::RuleBansGeneratedContent,
      Self::NoForegroundSubjectDetected,
      Self::FormatNotSupported,
      Self::GenerationFailed,
      Self::FaceNotDetected,
      Self::KeepAliveElapsed,
      Self::NotYetImplemented,
      Self::RetryableWorkerError,
      Self::ModelRulesViolation,
      Self::FilesizeTooLarge,
      Self::ImageDimensionsTooSmall,
      Self::ImageDimensionsTooLarge,
    ])
  }
}

#[cfg(test)]
mod tests {
  use crate::enums::task_failure_type::TaskFailureType;
  use crate::test_helpers::assert_serialization;

  mod explicit_checks {
    use super::*;
    use crate::enums::enum_error::EnumError;

    #[test]
    fn test_serialization() {
      assert_serialization(TaskFailureType::Unknown, "unknown");
      assert_serialization(TaskFailureType::RuleBansUserImage, "rule_bans_user_image");
      assert_serialization(TaskFailureType::RuleBansUserImageWithFaces, "rule_bans_user_image_with_faces");
      assert_serialization(TaskFailureType::RuleBansUserTextPrompt, "rule_bans_user_text_prompt");
      assert_serialization(TaskFailureType::RuleBansUserContent, "rule_bans_user_content");
      assert_serialization(TaskFailureType::RuleBansGeneratedVideo, "rule_bans_generated_video");
      assert_serialization(TaskFailureType::RuleBansGeneratedAudio, "rule_bans_generated_audio");
      assert_serialization(TaskFailureType::RuleBansGeneratedContent, "rule_bans_generated_content");
      assert_serialization(TaskFailureType::NoForegroundSubjectDetected, "no_foreground_subject_detected");
      assert_serialization(TaskFailureType::FormatNotSupported, "format_not_supported");
      assert_serialization(TaskFailureType::GenerationFailed, "generation_failed");
      assert_serialization(TaskFailureType::FaceNotDetected, "face_not_detected");
      assert_serialization(TaskFailureType::KeepAliveElapsed, "keep_alive_elapsed");
      assert_serialization(TaskFailureType::NotYetImplemented, "not_yet_implemented");
      assert_serialization(TaskFailureType::RetryableWorkerError, "retryable_worker_error");
      assert_serialization(TaskFailureType::ModelRulesViolation, "model_rules_violation");
      assert_serialization(TaskFailureType::FilesizeTooLarge, "filesize_too_large");
      assert_serialization(TaskFailureType::ImageDimensionsTooSmall, "image_dimensions_too_small");
      assert_serialization(TaskFailureType::ImageDimensionsTooLarge, "image_dimensions_too_large");
    }

    #[test]
    fn to_str() {
      assert_eq!(TaskFailureType::Unknown.to_str(), "unknown");
      assert_eq!(TaskFailureType::RuleBansUserImage.to_str(), "rule_bans_user_image");
      assert_eq!(TaskFailureType::RuleBansUserImageWithFaces.to_str(), "rule_bans_user_image_with_faces");
      assert_eq!(TaskFailureType::RuleBansUserTextPrompt.to_str(), "rule_bans_user_text_prompt");
      assert_eq!(TaskFailureType::RuleBansUserContent.to_str(), "rule_bans_user_content");
      assert_eq!(TaskFailureType::RuleBansGeneratedVideo.to_str(), "rule_bans_generated_video");
      assert_eq!(TaskFailureType::RuleBansGeneratedAudio.to_str(), "rule_bans_generated_audio");
      assert_eq!(TaskFailureType::RuleBansGeneratedContent.to_str(), "rule_bans_generated_content");
      assert_eq!(TaskFailureType::NoForegroundSubjectDetected.to_str(), "no_foreground_subject_detected");
      assert_eq!(TaskFailureType::FormatNotSupported.to_str(), "format_not_supported");
      assert_eq!(TaskFailureType::GenerationFailed.to_str(), "generation_failed");
      assert_eq!(TaskFailureType::FaceNotDetected.to_str(), "face_not_detected");
      assert_eq!(TaskFailureType::KeepAliveElapsed.to_str(), "keep_alive_elapsed");
      assert_eq!(TaskFailureType::NotYetImplemented.to_str(), "not_yet_implemented");
      assert_eq!(TaskFailureType::RetryableWorkerError.to_str(), "retryable_worker_error");
      assert_eq!(TaskFailureType::ModelRulesViolation.to_str(), "model_rules_violation");
      assert_eq!(TaskFailureType::FilesizeTooLarge.to_str(), "filesize_too_large");
      assert_eq!(TaskFailureType::ImageDimensionsTooSmall.to_str(), "image_dimensions_too_small");
      assert_eq!(TaskFailureType::ImageDimensionsTooLarge.to_str(), "image_dimensions_too_large");
    }

    #[test]
    fn from_str() {
      assert_eq!(TaskFailureType::from_str("unknown").unwrap(), TaskFailureType::Unknown);
      assert_eq!(TaskFailureType::from_str("rule_bans_user_image").unwrap(), TaskFailureType::RuleBansUserImage);
      assert_eq!(TaskFailureType::from_str("rule_bans_user_image_with_faces").unwrap(), TaskFailureType::RuleBansUserImageWithFaces);
      assert_eq!(TaskFailureType::from_str("rule_bans_user_text_prompt").unwrap(), TaskFailureType::RuleBansUserTextPrompt);
      assert_eq!(TaskFailureType::from_str("rule_bans_user_content").unwrap(), TaskFailureType::RuleBansUserContent);
      assert_eq!(TaskFailureType::from_str("rule_bans_generated_video").unwrap(), TaskFailureType::RuleBansGeneratedVideo);
      assert_eq!(TaskFailureType::from_str("rule_bans_generated_audio").unwrap(), TaskFailureType::RuleBansGeneratedAudio);
      assert_eq!(TaskFailureType::from_str("rule_bans_generated_content").unwrap(), TaskFailureType::RuleBansGeneratedContent);
      assert_eq!(TaskFailureType::from_str("no_foreground_subject_detected").unwrap(), TaskFailureType::NoForegroundSubjectDetected);
      assert_eq!(TaskFailureType::from_str("format_not_supported").unwrap(), TaskFailureType::FormatNotSupported);
      assert_eq!(TaskFailureType::from_str("generation_failed").unwrap(), TaskFailureType::GenerationFailed);
      assert_eq!(TaskFailureType::from_str("face_not_detected").unwrap(), TaskFailureType::FaceNotDetected);
      assert_eq!(TaskFailureType::from_str("keep_alive_elapsed").unwrap(), TaskFailureType::KeepAliveElapsed);
      assert_eq!(TaskFailureType::from_str("not_yet_implemented").unwrap(), TaskFailureType::NotYetImplemented);
      assert_eq!(TaskFailureType::from_str("retryable_worker_error").unwrap(), TaskFailureType::RetryableWorkerError);
      assert_eq!(TaskFailureType::from_str("model_rules_violation").unwrap(), TaskFailureType::ModelRulesViolation);
      assert_eq!(TaskFailureType::from_str("filesize_too_large").unwrap(), TaskFailureType::FilesizeTooLarge);
      assert_eq!(TaskFailureType::from_str("image_dimensions_too_small").unwrap(), TaskFailureType::ImageDimensionsTooSmall);
      assert_eq!(TaskFailureType::from_str("image_dimensions_too_large").unwrap(), TaskFailureType::ImageDimensionsTooLarge);
    }

    #[test]
    fn from_str_err() {
      let result = TaskFailureType::from_str("asdf");
      assert!(result.is_err());
      if let Err(EnumError::CouldNotConvertFromString(value)) = result {
        assert_eq!(value, "asdf");
      } else {
        panic!("Expected EnumError::CouldNotConvertFromString");
      }
    }

    #[test]
    fn all_variants() {
      let mut variants = TaskFailureType::all_variants();
      assert_eq!(variants.len(), 19);
      assert_eq!(variants.pop_first(), Some(TaskFailureType::Unknown));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansUserImage));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansUserImageWithFaces));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansUserTextPrompt));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansUserContent));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansGeneratedVideo));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansGeneratedAudio));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RuleBansGeneratedContent));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::NoForegroundSubjectDetected));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::FormatNotSupported));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::GenerationFailed));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::FaceNotDetected));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::KeepAliveElapsed));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::NotYetImplemented));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::RetryableWorkerError));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::ModelRulesViolation));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::FilesizeTooLarge));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::ImageDimensionsTooSmall));
      assert_eq!(variants.pop_first(), Some(TaskFailureType::ImageDimensionsTooLarge));
      assert_eq!(variants.pop_first(), None);
    }
  }

  mod mechanical_checks {
    use super::*;

    #[test]
    fn variant_length() {
      use strum::IntoEnumIterator;
      assert_eq!(TaskFailureType::all_variants().len(), TaskFailureType::iter().len());
    }

    #[test]
    fn round_trip() {
      for variant in TaskFailureType::all_variants() {
        assert_eq!(variant, TaskFailureType::from_str(variant.to_str()).unwrap());
        assert_eq!(variant, TaskFailureType::from_str(&format!("{}", variant)).unwrap());
        assert_eq!(variant, TaskFailureType::from_str(&format!("{:?}", variant)).unwrap());
      }
    }
  }
}
