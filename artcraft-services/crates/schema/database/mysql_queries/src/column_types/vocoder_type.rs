//! These denote the pretrained vocoders

use anyhow::anyhow;

use errors::AnyhowResult;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum VocoderType {
  /// Current state of the art vocoder
  #[serde(rename = "hifigan-superres")]
  #[sqlx(rename = "hifigan-superres")]
  HifiGanSuperResolution,

  /// Older WaveGlow vocoder
  WaveGlow,
}

impl VocoderType {
  pub fn to_str(&self) -> &'static str {
    match self {
      VocoderType::HifiGanSuperResolution => "hifigan-superres",
      VocoderType::WaveGlow => "waveglow",
    }
  }

  pub fn from_str(vocoder_type: &str) -> AnyhowResult<Self> {
    match vocoder_type {
      "hifigan-superres" => Ok(VocoderType::HifiGanSuperResolution),
      "waveglow" => Ok(VocoderType::WaveGlow),
      _ => Err(anyhow!("invalid value: {:?}", vocoder_type)),
    }
  }
}
