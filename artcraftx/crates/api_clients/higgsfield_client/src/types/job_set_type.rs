use crate::types::string_enum::string_enum;

string_enum! {
  /// Which model/pipeline a job set ran. This is the `type` on a job set and
  /// `job_set_type` on job status responses.
  JobSetType {
    /// Nano Banana Pro (the web app's name); `nano-banana-2` on the wire.
    NanoBanana2 => "nano_banana_2",
    /// Nano Banana 2 (the web app's name); `nano_banana_flash` on the wire.
    NanoBananaFlash => "nano_banana_flash",
    NanoBanana2Lite => "nano_banana_2_lite",
    GptImage2 => "gpt_image_2",
    SeedreamV5Pro => "seedream_v5_pro",
    SeedreamV5Lite => "seedream_v5_lite",
    SeedreamV4p5 => "seedream_v4_5",
    // ── Video ──
    Seedance2p5 => "seedance_2_5",
    Seedance2p0 => "seedance_2_0",
    Seedance2p0Mini => "seedance_2_0_mini",
    MinimaxH3 => "minimax_h3",
    Kling3p0 => "kling3_0",
    GrokVideoV15 => "grok_video_v15",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in JobSetType::known_variants() {
      assert_eq!(&JobSetType::from_str_lossy(variant.as_str()), variant);
    }
    let parsed: JobSetType = serde_json::from_str("\"seedance_2\"").unwrap();
    assert_eq!(parsed, JobSetType::Other("seedance_2".to_string()));
    assert_eq!(JobSetType::from_str_lossy("seedream_v4_5"), JobSetType::SeedreamV4p5);
    assert_eq!(JobSetType::from_str_lossy("nano_banana_flash"), JobSetType::NanoBananaFlash);
    assert_eq!(JobSetType::from_str_lossy("kling3_0"), JobSetType::Kling3p0);
    assert_eq!(JobSetType::from_str_lossy("grok_video_v15"), JobSetType::GrokVideoV15);
  }
}
