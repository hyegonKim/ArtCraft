use crate::achievements::user_badge_type::UserBadgeType;

/// User achievements
/// Numeric denominations are concrete types.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UserBadge {
  // Granted for early vocodes users.
  EarlyUser,

  // Granted for uploading voice conversion models
  VoiceConversionModelUploader1,
  VoiceConversionModelUploader5,
  VoiceConversionModelUploader10,
  VoiceConversionModelUploader20,
  VoiceConversionModelUploader50,
  VoiceConversionModelUploader100,
  VoiceConversionModelUploader150,
  VoiceConversionModelUploader200,
  VoiceConversionModelUploader250,
  VoiceConversionModelUploader500,
  VoiceConversionModelUploader1000,

  // Granted for uploading tts models
  TtsModelUploader1,
  TtsModelUploader5,
  TtsModelUploader10,
  TtsModelUploader20,
  TtsModelUploader50,
  TtsModelUploader100,
  TtsModelUploader150,
  TtsModelUploader200,
  TtsModelUploader250,
  TtsModelUploader500,
  TtsModelUploader1000,

  // Granted for vocoder models
  VocoderModelUploader1,
  VocoderModelUploader5,
  VocoderModelUploader10,
  VocoderModelUploader20,
  VocoderModelUploader50,
  VocoderModelUploader100,
  VocoderModelUploader150,
  VocoderModelUploader200,
  VocoderModelUploader250,
  VocoderModelUploader500,
  VocoderModelUploader1000,

  // Granted for vocoder (softvc aka "rocket_vc") models
  VocoderRocketVcModelUploader1,
  VocoderRocketVcModelUploader5,
  VocoderRocketVcModelUploader10,
  VocoderRocketVcModelUploader20,
  VocoderRocketVcModelUploader50,
  VocoderRocketVcModelUploader100,
  VocoderRocketVcModelUploader150,
  VocoderRocketVcModelUploader200,
  VocoderRocketVcModelUploader250,
  VocoderRocketVcModelUploader500,
  VocoderRocketVcModelUploader1000,

  // Granted for uploading templates
  W2lTemplateUploader1,
  W2lTemplateUploader10,
  W2lTemplateUploader50,
  W2lTemplateUploader100,
  W2lTemplateUploader200,
  W2lTemplateUploader500,
  W2lTemplateUploader1000,
  W2lTemplateUploader2000,
  W2lTemplateUploader5000,
  W2lTemplateUploader10000,

  //// Granted for using models
  //TtsInferenceUser100,
  //TtsInferenceUser500,
  //TtsInferenceUser1000,
  //TtsInferenceUser10000,
  //TtsInferenceUser100000,

  //// Granted for using templates
  //W2lInferenceUser100,
  //W2lInferenceUser500,
  //W2lInferenceUser1000,
  //W2lInferenceUser10000,
  //W2lInferenceUser100000,
}

impl UserBadge {
  /// This is the value that lives in the database as a VARCHAR(32)
  pub fn to_db_value(&self) -> &'static str {
    match self {
      UserBadge::EarlyUser => "early_user",

      UserBadge::VoiceConversionModelUploader1 => "vc_model_uploader_1",
      UserBadge::VoiceConversionModelUploader5 => "vc_model_uploader_5",
      UserBadge::VoiceConversionModelUploader10 => "vc_model_uploader_10",
      UserBadge::VoiceConversionModelUploader20 => "vc_model_uploader_20",
      UserBadge::VoiceConversionModelUploader50 => "vc_model_uploader_50",
      UserBadge::VoiceConversionModelUploader100 => "vc_model_uploader_100",
      UserBadge::VoiceConversionModelUploader150 => "vc_model_uploader_150",
      UserBadge::VoiceConversionModelUploader200 => "vc_model_uploader_200",
      UserBadge::VoiceConversionModelUploader250 => "vc_model_uploader_250",
      UserBadge::VoiceConversionModelUploader500 => "vc_model_uploader_500",
      UserBadge::VoiceConversionModelUploader1000 => "vc_model_uploader_1000",

      UserBadge::TtsModelUploader1 => "tts_model_uploader_1",
      UserBadge::TtsModelUploader5 => "tts_model_uploader_5",
      UserBadge::TtsModelUploader10 => "tts_model_uploader_10",
      UserBadge::TtsModelUploader20 => "tts_model_uploader_20",
      UserBadge::TtsModelUploader50 => "tts_model_uploader_50",
      UserBadge::TtsModelUploader100 => "tts_model_uploader_100",
      UserBadge::TtsModelUploader150 => "tts_model_uploader_150",
      UserBadge::TtsModelUploader200 => "tts_model_uploader_200",
      UserBadge::TtsModelUploader250 => "tts_model_uploader_250",
      UserBadge::TtsModelUploader500 => "tts_model_uploader_500",
      UserBadge::TtsModelUploader1000 => "tts_model_uploader_1000",

      UserBadge::VocoderModelUploader1 => "vocoder_model_uploader_1",
      UserBadge::VocoderModelUploader5 => "vocoder_model_uploader_5",
      UserBadge::VocoderModelUploader10 => "vocoder_model_uploader_10",
      UserBadge::VocoderModelUploader20 => "vocoder_model_uploader_20",
      UserBadge::VocoderModelUploader50 => "vocoder_model_uploader_50",
      UserBadge::VocoderModelUploader100 => "vocoder_model_uploader_100",
      UserBadge::VocoderModelUploader150 => "vocoder_model_uploader_150",
      UserBadge::VocoderModelUploader200 => "vocoder_model_uploader_200",
      UserBadge::VocoderModelUploader250 => "vocoder_model_uploader_250",
      UserBadge::VocoderModelUploader500 => "vocoder_model_uploader_500",
      UserBadge::VocoderModelUploader1000 => "vocoder_model_uploader_1000",

      UserBadge::VocoderRocketVcModelUploader1 => "vocoder_rocket_vc_uploader_1",
      UserBadge::VocoderRocketVcModelUploader5 => "vocoder_rocket_vc_uploader_5",
      UserBadge::VocoderRocketVcModelUploader10 => "vocoder_rocket_vc_uploader_10",
      UserBadge::VocoderRocketVcModelUploader20 => "vocoder_rocket_vc_uploader_20",
      UserBadge::VocoderRocketVcModelUploader50 => "vocoder_rocket_vc_uploader_50",
      UserBadge::VocoderRocketVcModelUploader100 => "vocoder_rocket_vc_uploader_100",
      UserBadge::VocoderRocketVcModelUploader150 => "vocoder_rocket_vc_uploader_150",
      UserBadge::VocoderRocketVcModelUploader200 => "vocoder_rocket_vc_uploader_200",
      UserBadge::VocoderRocketVcModelUploader250 => "vocoder_rocket_vc_uploader_250",
      UserBadge::VocoderRocketVcModelUploader500 => "vocoder_rocket_vc_uploader_500",
      UserBadge::VocoderRocketVcModelUploader1000 => "vocoder_rocket_vc_uploader_1000", // NB: 31 characters, table max is 32

      UserBadge::W2lTemplateUploader1 => "w2l_template_uploader_1",
      UserBadge::W2lTemplateUploader10 => "w2l_template_uploader_10",
      UserBadge::W2lTemplateUploader50 => "w2l_template_uploader_50",
      UserBadge::W2lTemplateUploader100 => "w2l_template_uploader_100",
      UserBadge::W2lTemplateUploader200 => "w2l_template_uploader_200",
      UserBadge::W2lTemplateUploader500 => "w2l_template_uploader_500",
      UserBadge::W2lTemplateUploader1000 => "w2l_template_uploader_1000",
      UserBadge::W2lTemplateUploader2000 => "w2l_template_uploader_2000",
      UserBadge::W2lTemplateUploader5000 => "w2l_template_uploader_5000",
      UserBadge::W2lTemplateUploader10000 => "w2l_template_uploader_10000",

      //UserBadge::TtsInferenceUser100 => "tts_inference_100",
      //UserBadge::TtsInferenceUser500 => "tts_inference_500",
      //UserBadge::TtsInferenceUser1000 => "tts_inference_1000",
      //UserBadge::TtsInferenceUser10000 =>"tts_inference_10000",
      //UserBadge::TtsInferenceUser100000 => "tts_inference_100000",
      //UserBadge::W2lInferenceUser100 => "w2l_inference_100",
      //UserBadge::W2lInferenceUser500 => "w2l_inference_500",
      //UserBadge::W2lInferenceUser1000 => "w2l_inference_1000",
      //UserBadge::W2lInferenceUser10000 => "w2l_inference_10000",
      //UserBadge::W2lInferenceUser100000 => "w2l_inference_100000",
    }
  }

  pub fn get_user_badge_type(&self) -> UserBadgeType {
    match self {
      UserBadge::EarlyUser => UserBadgeType::EarlyUser,

      UserBadge::VoiceConversionModelUploader1
      | UserBadge::VoiceConversionModelUploader5
      | UserBadge::VoiceConversionModelUploader10
      | UserBadge::VoiceConversionModelUploader20
      | UserBadge::VoiceConversionModelUploader50
      | UserBadge::VoiceConversionModelUploader100
      | UserBadge::VoiceConversionModelUploader150
      | UserBadge::VoiceConversionModelUploader200
      | UserBadge::VoiceConversionModelUploader250
      | UserBadge::VoiceConversionModelUploader500
      | UserBadge::VoiceConversionModelUploader1000
        => UserBadgeType::VoiceConversionModelUploader,

      UserBadge::TtsModelUploader1
        | UserBadge::TtsModelUploader5
        | UserBadge::TtsModelUploader10
        | UserBadge::TtsModelUploader20
        | UserBadge::TtsModelUploader50
        | UserBadge::TtsModelUploader100
        | UserBadge::TtsModelUploader150
        | UserBadge::TtsModelUploader200
        | UserBadge::TtsModelUploader250
        | UserBadge::TtsModelUploader500
        | UserBadge::TtsModelUploader1000
          => UserBadgeType::TtsModelUploader,

      UserBadge::VocoderModelUploader1
        | UserBadge::VocoderModelUploader5
        | UserBadge::VocoderModelUploader10
        | UserBadge::VocoderModelUploader20
        | UserBadge::VocoderModelUploader50
        | UserBadge::VocoderModelUploader100
        | UserBadge::VocoderModelUploader150
        | UserBadge::VocoderModelUploader200
        | UserBadge::VocoderModelUploader250
        | UserBadge::VocoderModelUploader500
        | UserBadge::VocoderModelUploader1000
          => UserBadgeType::VocoderModelUploader,

      UserBadge::VocoderRocketVcModelUploader1
        | UserBadge::VocoderRocketVcModelUploader5
        | UserBadge::VocoderRocketVcModelUploader10
        | UserBadge::VocoderRocketVcModelUploader20
        | UserBadge::VocoderRocketVcModelUploader50
        | UserBadge::VocoderRocketVcModelUploader100
        | UserBadge::VocoderRocketVcModelUploader150
        | UserBadge::VocoderRocketVcModelUploader200
        | UserBadge::VocoderRocketVcModelUploader250
        | UserBadge::VocoderRocketVcModelUploader500
        | UserBadge::VocoderRocketVcModelUploader1000
          => UserBadgeType::VocoderRocketVcModelUploader,

      UserBadge::W2lTemplateUploader1
        | UserBadge::W2lTemplateUploader10
        | UserBadge::W2lTemplateUploader50
        | UserBadge::W2lTemplateUploader100
        | UserBadge::W2lTemplateUploader200
        | UserBadge::W2lTemplateUploader500
        | UserBadge::W2lTemplateUploader1000
        | UserBadge::W2lTemplateUploader2000
        | UserBadge::W2lTemplateUploader5000
        | UserBadge::W2lTemplateUploader10000
          => UserBadgeType::W2lTemplateUploader,
    }
  }
}
