use router::api::router_aspect_ratio::RouterAspectRatio;
use router::api::router_resolution::RouterResolution;
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use serde_derive::{Deserialize, Serialize};
use artcraft_client::tokens::characters::CharacterToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::tauri_media_source::{
  merge_source_with_legacy_token, merge_sources_with_legacy_tokens, TauriMediaSource,
};
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

/// This is used in the Tauri command bridge.
/// Don't change the serializations without coordinating with the frontend.
///
/// MIGRATION (2026-07): we're moving the frontend to send storyteller-web omni
/// identifiers (`CommonVideoModel` serde strings). Variants whose legacy Tauri
/// id differs carry the omni id as a `#[serde(alias)]` so BOTH deserialize.
/// Once the frontend is 100% omni ids, the legacy renames can be dropped.
#[derive(Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum TauriVideoModel {
  #[serde(rename = "grok_video", alias = "grok_imagine_video")]
  GrokVideo,

  #[serde(rename = "grok_imagine_video_1p5")]
  GrokImagineVideo1p5,

  #[serde(rename = "kling_1.6_pro", alias = "kling_1p6_pro")]
  Kling16Pro,

  #[serde(rename = "kling_2.1_pro", alias = "kling_2p1_pro")]
  Kling21Pro,

  #[serde(rename = "kling_2.1_master", alias = "kling_2p1_master")]
  Kling21Master,

  #[serde(rename = "kling_2p5_turbo_pro")]
  Kling2p5TurboPro,

  #[serde(rename = "kling_2p6_pro")]
  Kling2p6Pro,

  #[serde(rename = "kling_3p0_standard")]
  Kling3p0Standard,

  #[serde(rename = "kling_3p0_pro")]
  Kling3p0Pro,

  #[serde(rename = "happy_horse_1p0")]
  HappyHorse1p0,

  #[serde(rename = "seedance_1.0_lite", alias = "seedance_1p0_lite")]
  Seedance10Lite,

  #[serde(rename = "seedance_1p5_pro")]
  Seedance1p5Pro,

  #[serde(rename = "seedance_2p0")]
  Seedance2p0,

  #[serde(rename = "seedance_2p0_fast")]
  Seedance2p0Fast,

  #[serde(rename = "seedance_2p0_mini")]
  Seedance2p0Mini,

  #[serde(rename = "seedance_2p5")]
  Seedance2p5,

  #[serde(rename = "seedance_2p5_edit")]
  Seedance2p5Edit,

  #[serde(rename = "minimax_h3")]
  MinimaxH3,

  #[serde(rename = "sora_2")]
  Sora2,

  #[serde(rename = "sora_2_pro")]
  Sora2Pro,

  #[serde(rename = "veo_2")]
  Veo2,

  #[serde(rename = "veo_3")]
  Veo3,

  #[serde(rename = "veo_3_fast")]
  Veo3Fast,

  #[serde(rename = "veo_3p1")]
  Veo3p1,

  #[serde(rename = "veo_3p1_fast")]
  Veo3p1Fast,

  #[serde(rename = "veo_3p1_lite")]
  Veo3p1Lite,

  #[serde(rename = "vidu_q3")]
  ViduQ3,

  #[serde(rename = "vidu_q3_turbo")]
  ViduQ3Turbo,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct TauriGenerateVideoRequest {
  /// Stable id (`credential_{entropy}`) of the stored credential (account)
  /// to generate with. Loaded from disk; generation routes to the
  /// credential's service.
  pub credential_id: Option<String>,

  pub provider: Option<GenerationSource>,
  pub model: Option<TauriVideoModel>,

  pub prompt: Option<String>,
  pub negative_prompt: Option<String>,

  #[deprecated(note = "Use start_frame_image_media_token instead")]
  pub image_media_token: Option<MediaFileToken>,

  pub start_frame_image_media_token: Option<MediaFileToken>,
  pub end_frame_image_media_token: Option<MediaFileToken>,

  pub reference_image_media_tokens: Option<Vec<MediaFileToken>>,
  pub reference_video_media_tokens: Option<Vec<MediaFileToken>>,
  pub reference_audio_media_tokens: Option<Vec<MediaFileToken>>,
  pub reference_character_tokens: Option<Vec<CharacterToken>>,

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy `*_media_token(s)` twin above when both are set; local
  // files and bytes never touch the ArtCraft cloud unless the target
  // provider requires it.
  pub start_frame_image_source: Option<TauriMediaSource>,
  pub end_frame_image_source: Option<TauriMediaSource>,
  pub reference_image_sources: Option<Vec<TauriMediaSource>>,
  pub reference_video_sources: Option<Vec<TauriMediaSource>>,
  pub reference_audio_sources: Option<Vec<TauriMediaSource>>,

  pub aspect_ratio: Option<RouterAspectRatio>,
  pub resolution: Option<RouterResolution>,

  pub duration_seconds: Option<u16>,
  pub generate_audio: Option<bool>,
  pub video_batch_count: Option<u16>,

  #[deprecated(note = "Use start_frame_image_media_token instead")]
  pub sora_orientation: Option<SoraOrientation>,

  #[deprecated(note = "Use start_frame_image_media_token instead")]
  pub grok_aspect_ratio: Option<GrokAspectRatio>,

  pub frontend_caller: Option<TauriCommandCaller>,
  pub frontend_subscriber_id: Option<String>,
  pub frontend_subscriber_payload: Option<String>,
}

/// The request's media inputs, normalized: legacy token fields are folded
/// into [`TauriMediaSource`]s so handlers see one shape.
#[derive(Debug, Clone)]
pub struct VideoRequestMediaSources {
  pub start_frame: Option<TauriMediaSource>,
  pub end_frame: Option<TauriMediaSource>,
  pub reference_images: Option<Vec<TauriMediaSource>>,
  pub reference_videos: Option<Vec<TauriMediaSource>>,
  pub reference_audios: Option<Vec<TauriMediaSource>>,
}

impl TauriGenerateVideoRequest {
  pub fn media_sources(&self) -> VideoRequestMediaSources {
    VideoRequestMediaSources {
      start_frame: merge_source_with_legacy_token(
        self.start_frame_image_source.clone(),
        self.start_frame_image_media_token.clone(),
      ),
      end_frame: merge_source_with_legacy_token(
        self.end_frame_image_source.clone(),
        self.end_frame_image_media_token.clone(),
      ),
      reference_images: merge_sources_with_legacy_tokens(
        self.reference_image_sources.clone(),
        self.reference_image_media_tokens.clone(),
      ),
      reference_videos: merge_sources_with_legacy_tokens(
        self.reference_video_sources.clone(),
        self.reference_video_media_tokens.clone(),
      ),
      reference_audios: merge_sources_with_legacy_tokens(
        self.reference_audio_sources.clone(),
        self.reference_audio_media_tokens.clone(),
      ),
    }
  }
}

impl VideoRequestMediaSources {
  /// Every source the request carries, for up-front validation and for
  /// collecting the tokens that need CDN resolution.
  pub fn iter(&self) -> impl Iterator<Item = &TauriMediaSource> {
    self.start_frame.iter()
        .chain(self.end_frame.iter())
        .chain(self.reference_images.iter().flatten())
        .chain(self.reference_videos.iter().flatten())
        .chain(self.reference_audios.iter().flatten())
  }
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum SoraOrientation {
  Portrait,
  Landscape,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum GrokAspectRatio {
  Portrait,
  Landscape,
  Square,
}

#[derive(Serialize)]
pub struct TauriGenerateVideoResponse {
}

impl SerializeMarker for TauriGenerateVideoResponse {}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TauriGenerateVideoErrorType {
  ModelNotSpecified,
  NoProviderAvailable,
  ServerError,
  NeedsFalApiKey,
  FalError,
  NeedsStorytellerCredentials,
  /// Problem with the selected account credential (absent, unknown, or
  /// unusable). The backend also flashes a dismissable modal.
  CredentialProblem,
}
