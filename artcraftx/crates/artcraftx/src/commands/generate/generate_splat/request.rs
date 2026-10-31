use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use serde_derive::{Deserialize, Serialize};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::tauri_media_source::{
  merge_source_with_legacy_token, merge_sources_with_legacy_tokens, TauriMediaSource,
};
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

// ── Request ──

#[derive(Deserialize, Debug)]
pub struct TauriGenerateSplatRequest {
  /// Stable id (`credential_{entropy}`) of the stored credential (account)
  /// to generate with. Loaded from disk; generation routes to the
  /// credential's service.
  pub credential_id: Option<String>,

  /// The model to use.
  pub model: Option<TauriSplatModel>,

  /// Text prompt.
  pub prompt: Option<String>,

  /// Reference images (already uploaded).
  pub reference_image_media_tokens: Option<Vec<MediaFileToken>>,

  /// Reference video (already uploaded).
  pub reference_video_media_token: Option<MediaFileToken>,

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy `*_media_token(s)` twin above when both are set.
  pub reference_image_sources: Option<Vec<TauriMediaSource>>,
  pub reference_video_source: Option<TauriMediaSource>,

  /// The reference image is a 360-degree panorama.
  pub is_panoramic: Option<bool>,

  /// Disable server-side recaptioning of the prompt.
  pub disable_recaption: Option<bool>,

  // ── Frontend metadata ──

  /// Name of the frontend caller.
  pub frontend_caller: Option<TauriCommandCaller>,

  /// A frontend-defined identifier sent back as a Tauri event on task completion.
  pub frontend_subscriber_id: Option<String>,

  /// A frontend-defined payload sent back as a Tauri event on task completion.
  pub frontend_subscriber_payload: Option<String>,
}

/// The splat request's media inputs, normalized: legacy token fields fold
/// into [`TauriMediaSource`]s so handlers see one shape.
#[derive(Debug, Clone)]
pub struct SplatRequestMediaSources {
  pub reference_images: Option<Vec<TauriMediaSource>>,
  pub reference_video: Option<TauriMediaSource>,
}

impl TauriGenerateSplatRequest {
  pub fn media_sources(&self) -> SplatRequestMediaSources {
    SplatRequestMediaSources {
      reference_images: merge_sources_with_legacy_tokens(
        self.reference_image_sources.clone(), self.reference_image_media_tokens.clone(),
      ),
      reference_video: merge_source_with_legacy_token(
        self.reference_video_source.clone(), self.reference_video_media_token.clone(),
      ),
    }
  }
}

impl SplatRequestMediaSources {
  /// Every source the request carries, for up-front validation.
  pub fn iter(&self) -> impl Iterator<Item = &TauriMediaSource> {
    self.reference_images.iter().flatten().chain(self.reference_video.iter())
  }
}

/// The splat models the frontend can request, identified by their omni
/// model ids (`CommonSplatModel` serde strings).
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum TauriSplatModel {
  #[serde(rename = "marble_0p1_mini")]
  Marble0p1Mini,
  #[serde(rename = "marble_0p1_plus")]
  Marble0p1Plus,
  #[serde(rename = "marble_1p0")]
  Marble1p0,
  #[serde(rename = "marble_1p0_draft")]
  Marble1p0Draft,
  #[serde(rename = "marble_1p1")]
  Marble1p1,
  #[serde(rename = "marble_1p1_plus")]
  Marble1p1Plus,
  #[serde(rename = "triposplat")]
  TripoSplat,
}

// ── Response ──

#[derive(Serialize)]
pub struct TauriGenerateSplatResponse {
}

impl SerializeMarker for TauriGenerateSplatResponse {}

// ── Error ──

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TauriGenerateSplatErrorType {
  /// Caller didn't specify a model
  ModelNotSpecified,
  /// Generic server error
  ServerError,
  /// Problem with the selected account credential (absent, unknown, or
  /// unusable). The backend also flashes a dismissable modal.
  CredentialProblem,
}
