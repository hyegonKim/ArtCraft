use anyhow::bail;

use errors::AnyhowResult;

use crate::payloads::media_file_extra_info::inner_payloads::face_fusion_video_extra_info::FaceFusionVideoExtraInfo;
use crate::payloads::media_file_extra_info::inner_payloads::live_portrait_video_extra_info::LivePortraitVideoExtraInfo;
use crate::payloads::media_file_extra_info::inner_payloads::stable_diffusion_extra_info::StableDiffusionExtraInfo;

/// For things that don't fit in the schema
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MediaFileExtraInfo {
  /// Face Fusion Video
  /// NB: Enum variant is short to conserve DB space.
  /// NB: DO NOT CHANGE. It could break live jobs.
  F(FaceFusionVideoExtraInfo),

  /// Live Portrait Video
  /// NB: Enum variant is short to conserve DB space.
  /// NB: DO NOT CHANGE. It could break live jobs.
  L(LivePortraitVideoExtraInfo),

  /// Legacy Stable Diffusion info we used to write (though we
  /// did not use nested polymorphic enums structs for the JSON)
  /// If we do stable diffusion again, it's probably best to
  /// design a new and more efficient/compact struct.
  S(StableDiffusionExtraInfo),
}

impl MediaFileExtraInfo {
  pub fn from_json_str(value: &str) -> AnyhowResult<Self> {
    let result = serde_json::from_str(value);
    let err = match result {
      Ok(value) => return Ok(value),
      Err(err) => err,
    };
    // NB: Some older stable diffusion payloads were raw equivalents of `StableDiffusionExtraInfo`.
    if value.contains("prompt") {
      let stable_diffusion_info : StableDiffusionExtraInfo = serde_json::from_str(value)?;
      return Ok(Self::S(stable_diffusion_info));
    } else {
      return bail!(err);
    }
  }

  pub fn to_json_string(&self) -> AnyhowResult<String> {
    Ok(serde_json::to_string(self)?)
  }
}
