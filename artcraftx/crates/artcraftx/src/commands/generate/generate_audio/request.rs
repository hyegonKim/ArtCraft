use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use serde_derive::{Deserialize, Serialize};
use artcraft_client::enums::common::generation::common_musical_key::CommonMusicalKey;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::tauri_media_source::{merge_sources_with_legacy_tokens, TauriMediaSource};
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

// ── Request ──

#[derive(Deserialize, Debug)]
pub struct TauriGenerateAudioRequest {
  /// Stable id (`credential_{entropy}`) of the stored credential (account)
  /// to generate with. Loaded from disk; generation routes to the
  /// credential's service.
  pub credential_id: Option<String>,

  /// The model to use.
  pub model: Option<TauriAudioModel>,

  /// Text prompt.
  pub prompt: Option<String>,

  /// Style/genre prompt (Suno's "tags").
  pub style_prompt: Option<String>,

  /// Reference audio (already uploaded).
  pub audio_media_tokens: Option<Vec<MediaFileToken>>,

  /// Reference images (already uploaded).
  pub image_media_tokens: Option<Vec<MediaFileToken>>,

  /// Three-way reference audio (bytes | local path | media token). Wins
  /// over `audio_media_tokens` when both are set.
  pub audio_sources: Option<Vec<TauriMediaSource>>,

  /// Three-way reference images (bytes | local path | media token). Wins
  /// over `image_media_tokens` when both are set.
  pub image_sources: Option<Vec<TauriMediaSource>>,

  /// Keep the original lyrics (Suno Remix).
  pub keep_lyrics: Option<bool>,

  /// Instrumental-only toggle (Suno Music / Sample).
  pub is_instrumental: Option<bool>,

  /// Loopable vs single-hit toggle (Suno Sounds).
  pub is_loopable: Option<bool>,

  /// Target BPM (Suno Sounds).
  pub bpm: Option<u16>,

  /// Musical key (Suno Sounds).
  pub musical_key: Option<CommonMusicalKey>,

  /// Sample rate in Hz (Seed Audio).
  pub sample_rate_hz: Option<u32>,

  /// Playback speed (Seed Audio, 0.5–2.0).
  pub speed: Option<f64>,

  /// Volume (Seed Audio, 0.5–2.0).
  pub volume: Option<f64>,

  /// Pitch shift in semitones (Seed Audio, -12..=12).
  pub pitch: Option<f64>,

  // ── Frontend metadata ──

  /// Name of the frontend caller.
  pub frontend_caller: Option<TauriCommandCaller>,

  /// A frontend-defined identifier sent back as a Tauri event on task completion.
  pub frontend_subscriber_id: Option<String>,

  /// A frontend-defined payload sent back as a Tauri event on task completion.
  pub frontend_subscriber_payload: Option<String>,
}

impl TauriGenerateAudioRequest {
  /// Reference audio, normalized: `audio_sources` wins, legacy tokens fold in.
  pub fn audio_media_sources(&self) -> Option<Vec<TauriMediaSource>> {
    merge_sources_with_legacy_tokens(self.audio_sources.clone(), self.audio_media_tokens.clone())
  }

  /// Reference images, normalized: `image_sources` wins, legacy tokens fold in.
  pub fn image_media_sources(&self) -> Option<Vec<TauriMediaSource>> {
    merge_sources_with_legacy_tokens(self.image_sources.clone(), self.image_media_tokens.clone())
  }
}

/// The audio models the frontend can request, identified by their omni
/// model ids (`CommonAudioModel` serde strings).
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum TauriAudioModel {
  #[serde(rename = "suno_music")]
  SunoMusic,
  #[serde(rename = "suno_remix")]
  SunoRemix,
  #[serde(rename = "suno_sounds")]
  SunoSounds,
  #[serde(rename = "suno_sample")]
  SunoSample,
  #[serde(rename = "seed_audio_1p0")]
  SeedAudio1p0,
}

// ── Response ──

#[derive(Serialize)]
pub struct TauriGenerateAudioResponse {
}

impl SerializeMarker for TauriGenerateAudioResponse {}

// ── Error ──

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TauriGenerateAudioErrorType {
  /// Caller didn't specify a model
  ModelNotSpecified,
  /// Generic server error
  ServerError,
  /// Problem with the selected account credential (absent, unknown, or
  /// unusable). The backend also flashes a dismissable modal.
  CredentialProblem,
}
