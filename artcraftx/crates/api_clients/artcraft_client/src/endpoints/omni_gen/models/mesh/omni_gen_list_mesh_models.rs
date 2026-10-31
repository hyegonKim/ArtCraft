//! Self-contained, forward-compatible binding for `GET /v1/omni_gen/models/mesh`.
//!
//! IMPORTANT: This module deliberately does NOT reuse any types from
//! `api_defs`. Client builds may be deployed in the wild long after the
//! server's models, enums, and response shapes have changed, so every request /
//! response type is copied here and made permissive:
//!   - Unknown JSON fields are ignored (serde default behavior).
//!   - Missing collection/flag fields default rather than fail (`serde(default)`).
//!   - Every string-valued enum has an `Unknown(String)` catch-all so new server
//!     variants deserialize instead of erroring.

use serde_derive::{Deserialize, Serialize};

use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::datatypes::common_enums::generation::api_client_generation_provider::ApiClientGenerationProvider;
use crate::datatypes::common_enums::generation::api_client_mesh_model::ApiClientMeshModel;
use crate::datatypes::common_enums::generation::api_client_mesh_output_type::ApiClientMeshOutputType;
use crate::datatypes::common_enums::generation::api_client_polygon_type::ApiClientPolygonType;
use crate::datatypes::common_enums::generation::api_client_model_creator::ApiClientModelCreator;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::basic_json_get_request::basic_json_get_request;

pub const OMNI_GEN_MESH_MODELS_PATH: &str = "/v1/omni_gen/models/mesh";

/// Arguments for [`omni_gen_list_mesh_models`].
pub struct OmniGenListMeshModelsArgs<'a> {
  pub api_host: &'a ApiHost,
  pub maybe_creds: Option<&'a StorytellerCredentialSet>,
}

/// List available mesh models.
pub async fn omni_gen_list_mesh_models(
  args: OmniGenListMeshModelsArgs<'_>,
) -> Result<OmniGenMeshModelsResponse, StorytellerError> {
  Ok(basic_json_get_request(
    args.api_host,
    OMNI_GEN_MESH_MODELS_PATH,
    args.maybe_creds,
  ).await?)
}

// ============================ Response types ============================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenMeshModelsResponse {
  #[serde(default)]
  pub success: bool,
  #[serde(default)]
  pub models: Vec<OmniGenMeshModelDetails>,
  #[serde(default)]
  pub providers: Vec<OmniGenMeshModelProviderDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenMeshModelProviderDetails {
  pub provider: ApiClientGenerationProvider,
  #[serde(default)]
  pub models: Vec<OmniGenMeshProviderModelDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenMeshProviderModelDetails {
  pub model: ApiClientMeshModel,
  #[serde(default)]
  pub overrides: Option<OmniGenMeshModelDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenMeshModelDetails {
  pub model: ApiClientMeshModel,
  #[serde(default)]
  pub model_creator: Option<ApiClientModelCreator>,
  #[serde(default)]
  pub full_name: Option<String>,
  #[serde(default)]
  pub extra_info: Option<String>,
  #[serde(default)]
  pub extra_info_short: Option<String>,
  #[serde(default)]
  pub text_prompt_supported: Option<bool>,
  #[serde(default)]
  pub image_input_supported: Option<bool>,
  #[serde(default)]
  pub sketch_input_supported: Option<bool>,
  #[serde(default)]
  pub multi_view_supported: Option<bool>,
  #[serde(default)]
  pub mesh_input_supported: Option<bool>,
  #[serde(default)]
  pub mesh_output_types: Option<Vec<ApiClientMeshOutputType>>,
  #[serde(default)]
  pub polygon_types: Option<Vec<ApiClientPolygonType>>,
  #[serde(default)]
  pub face_count_supported: Option<bool>,
  #[serde(default)]
  pub pbr_supported: Option<bool>,
  #[serde(default)]
  pub texture_toggle_supported: Option<bool>,
  #[serde(default)]
  pub texture_quality_supported: Option<bool>,
  #[serde(default)]
  pub geometry_quality_supported: Option<bool>,
  #[serde(default)]
  pub is_disabled: Option<bool>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn response_tolerates_unknown_fields_and_variants() {
    let json = r#"{
      "success": true,
      "models": [
        {
          "model": "brand_new_mesh_model",
          "model_creator": "some_new_studio",
          "surprise_new_field": 123
        }
      ],
      "providers": [
        { "provider": "new_provider", "models": [ { "model": "hunyuan_3d_3" } ] }
      ],
      "another_unexpected_top_level_field": "ignored"
    }"#;

    let resp: OmniGenMeshModelsResponse = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.models.len(), 1);
    assert_eq!(
      resp.models[0].model,
      ApiClientMeshModel::Unknown("brand_new_mesh_model".to_string()),
    );
    assert_eq!(
      resp.models[0].model_creator,
      Some(ApiClientModelCreator::Unknown("some_new_studio".to_string())),
    );
    assert_eq!(resp.models[0].is_disabled, None);
    assert_eq!(
      resp.providers[0].provider,
      ApiClientGenerationProvider::Unknown("new_provider".to_string()),
    );
  }

  #[test]
  fn empty_object_uses_defaults() {
    let resp: OmniGenMeshModelsResponse = serde_json::from_str("{}").unwrap();
    assert!(!resp.success);
    assert!(resp.models.is_empty());
    assert!(resp.providers.is_empty());
  }
}

/// Live tests that hit real servers. All `#[ignore]` so they never run by default.
/// Run explicitly, e.g.:
///   cargo test -p artcraft_client omni_gen_list_mesh_models::live_tests -- --ignored --nocapture
#[cfg(test)]
mod live_tests {
  mod localhost {
    use super::super::{omni_gen_list_mesh_models, OmniGenListMeshModelsArgs};
    use crate::utils::api_host::ApiHost;

    #[tokio::test]
    #[ignore] // live: hits http://localhost:12345
    async fn list_mesh_models() {
      let api_host = ApiHost::Localhost { port: 12345 };
      let response = omni_gen_list_mesh_models(OmniGenListMeshModelsArgs {
        api_host: &api_host,
        maybe_creds: None,
      })
      .await
      .expect("request should succeed");

      println!("[localhost] mesh models response: {:#?}", response);
      assert!(response.success);
      assert!(!response.models.is_empty(), "expected at least one mesh model");
    }
  }

  mod production {
    use super::super::{omni_gen_list_mesh_models, OmniGenListMeshModelsArgs};
    use crate::utils::api_host::ApiHost;

    #[tokio::test]
    #[ignore] // live: hits https://api.storyteller.ai
    async fn list_mesh_models() {
      let api_host = ApiHost::Storyteller;
      let response = omni_gen_list_mesh_models(OmniGenListMeshModelsArgs {
        api_host: &api_host,
        maybe_creds: None,
      })
      .await
      .expect("request should succeed");

      println!("[production] mesh models response: {:#?}", response);
      assert!(response.success);
      assert!(!response.models.is_empty(), "expected at least one mesh model");
    }
  }
}
