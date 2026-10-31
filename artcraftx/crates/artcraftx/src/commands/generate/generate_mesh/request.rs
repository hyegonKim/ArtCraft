use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use serde_derive::{Deserialize, Serialize};
use artcraft_client::enums::common::generation::common_mesh_output_type::CommonMeshOutputType;
use artcraft_client::enums::common::generation::common_mesh_quality::CommonMeshQuality;
use artcraft_client::enums::common::generation::common_polygon_type::CommonPolygonType;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::tauri_media_source::{
  merge_source_with_legacy_token, merge_sources_with_legacy_tokens, TauriMediaSource,
};
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;

// ── Request ──

#[derive(Deserialize, Debug)]
pub struct TauriGenerateMeshRequest {
  /// Stable id (`credential_{entropy}`) of the stored credential (account)
  /// to generate with. Loaded from disk; generation routes to the
  /// credential's service.
  pub credential_id: Option<String>,

  /// The model to use.
  pub model: Option<TauriMeshModel>,

  /// Text prompt.
  pub prompt: Option<String>,

  /// Reference images (already uploaded).
  pub reference_image_media_tokens: Option<Vec<MediaFileToken>>,

  /// Multi-view images (already uploaded).
  pub front_image_media_token: Option<MediaFileToken>,
  pub back_image_media_token: Option<MediaFileToken>,
  pub left_image_media_token: Option<MediaFileToken>,
  pub right_image_media_token: Option<MediaFileToken>,

  /// Input mesh for mesh-to-mesh models (part splitting, retopology).
  pub input_mesh_media_token: Option<MediaFileToken>,

  // Three-way media sources (bytes | local path | media token). Each wins
  // over its legacy `*_media_token(s)` twin above when both are set.
  pub reference_image_sources: Option<Vec<TauriMediaSource>>,
  pub front_image_source: Option<TauriMediaSource>,
  pub back_image_source: Option<TauriMediaSource>,
  pub left_image_source: Option<TauriMediaSource>,
  pub right_image_source: Option<TauriMediaSource>,
  pub input_mesh_source: Option<TauriMediaSource>,

  /// Requested mesh output type.
  pub mesh_output_type: Option<CommonMeshOutputType>,

  /// Requested polygon type.
  pub polygon_type: Option<CommonPolygonType>,

  /// Target face count.
  pub face_count: Option<u64>,

  /// Generate PBR materials.
  pub enable_pbr: Option<bool>,

  /// Generate textures (off = untextured output).
  pub enable_texture: Option<bool>,

  /// Texture quality level.
  pub texture_quality: Option<CommonMeshQuality>,

  /// Geometry quality level.
  pub geometry_quality: Option<CommonMeshQuality>,

  // ── Frontend metadata ──

  /// Name of the frontend caller.
  pub frontend_caller: Option<TauriCommandCaller>,

  /// A frontend-defined identifier sent back as a Tauri event on task completion.
  pub frontend_subscriber_id: Option<String>,

  /// A frontend-defined payload sent back as a Tauri event on task completion.
  pub frontend_subscriber_payload: Option<String>,
}

/// The mesh request's media inputs, normalized: legacy token fields fold
/// into [`TauriMediaSource`]s so handlers see one shape.
#[derive(Debug, Clone)]
pub struct MeshRequestMediaSources {
  pub reference_images: Option<Vec<TauriMediaSource>>,
  pub front_image: Option<TauriMediaSource>,
  pub back_image: Option<TauriMediaSource>,
  pub left_image: Option<TauriMediaSource>,
  pub right_image: Option<TauriMediaSource>,
  pub input_mesh: Option<TauriMediaSource>,
}

impl TauriGenerateMeshRequest {
  pub fn media_sources(&self) -> MeshRequestMediaSources {
    MeshRequestMediaSources {
      reference_images: merge_sources_with_legacy_tokens(
        self.reference_image_sources.clone(), self.reference_image_media_tokens.clone(),
      ),
      front_image: merge_source_with_legacy_token(self.front_image_source.clone(), self.front_image_media_token.clone()),
      back_image: merge_source_with_legacy_token(self.back_image_source.clone(), self.back_image_media_token.clone()),
      left_image: merge_source_with_legacy_token(self.left_image_source.clone(), self.left_image_media_token.clone()),
      right_image: merge_source_with_legacy_token(self.right_image_source.clone(), self.right_image_media_token.clone()),
      input_mesh: merge_source_with_legacy_token(self.input_mesh_source.clone(), self.input_mesh_media_token.clone()),
    }
  }
}

impl MeshRequestMediaSources {
  /// Every source the request carries, for up-front validation.
  pub fn iter(&self) -> impl Iterator<Item = &TauriMediaSource> {
    self.reference_images.iter().flatten()
        .chain(self.front_image.iter())
        .chain(self.back_image.iter())
        .chain(self.left_image.iter())
        .chain(self.right_image.iter())
        .chain(self.input_mesh.iter())
  }
}

/// The mesh models the frontend can request, identified by their omni
/// model ids (`CommonMeshModel` serde strings).
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum TauriMeshModel {
  #[serde(rename = "hunyuan_3d_2p0", alias = "hunyuan_3d_2_0", alias = "hunyuan_3d_2")]
  Hunyuan3d2p0,
  #[serde(rename = "hunyuan_3d_2p1", alias = "hunyuan_3d_2_1")]
  Hunyuan3d2p1,
  #[serde(rename = "hunyuan_3d_3")]
  Hunyuan3d3,
  #[serde(rename = "hunyuan_3d_3_sketch")]
  Hunyuan3d3Sketch,
  #[serde(rename = "hunyuan_3d_3p1_pro")]
  Hunyuan3d3p1Pro,
  #[serde(rename = "hunyuan_3d_3p1_rapid")]
  Hunyuan3d3p1Rapid,
  #[serde(rename = "hunyuan_3d_3p1_part")]
  Hunyuan3d3p1Part,
  #[serde(rename = "hunyuan_3d_3p1_topology")]
  Hunyuan3d3p1SmartTopology,
  #[serde(rename = "tripo3d_h3p1")]
  Tripo3dH3p1,
  #[serde(rename = "meshy_v6")]
  MeshyV6,
  #[serde(rename = "rodin_2p5_fast")]
  Rodin2p5Fast,
}

// ── Response ──

#[derive(Serialize)]
pub struct TauriGenerateMeshResponse {
}

impl SerializeMarker for TauriGenerateMeshResponse {}

// ── Error ──

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TauriGenerateMeshErrorType {
  /// Caller didn't specify a model
  ModelNotSpecified,
  /// Generic server error
  ServerError,
  /// Problem with the selected account credential (absent, unknown, or
  /// unusable). The backend also flashes a dismissable modal.
  CredentialProblem,
}
