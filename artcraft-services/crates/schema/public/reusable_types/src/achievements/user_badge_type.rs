
/// Numeric denominations are concrete types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UserBadgeType {
  /// Granted for early vocodes users
  EarlyUser,

  /// Granted for uploading voice conversion models
  VoiceConversionModelUploader,

  /// Granted for uploading tts models
  TtsModelUploader,

  /// Granted for uploading vocoder models
  VocoderModelUploader,

  /// Granted for uploading vocoder (softvc aka "rocket_vc") models
  VocoderRocketVcModelUploader,

  /// Granted for uploading W2L templates
  W2lTemplateUploader,
}
