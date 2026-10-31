//! Self-contained, forward-compatible binding for `GET /v1/omni_gen/models/splat`.
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
use crate::datatypes::common_enums::generation::api_client_splat_model::ApiClientSplatModel;
use crate::datatypes::common_enums::generation::api_client_model_creator::ApiClientModelCreator;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::basic_json_get_request::basic_json_get_request;

pub const OMNI_GEN_SPLAT_MODELS_PATH: &str = "/v1/omni_gen/models/splat";

/// Arguments for [`omni_gen_list_splat_models`].
pub struct OmniGenListSplatModelsArgs<'a> {
  pub api_host: &'a ApiHost,
  pub maybe_creds: Option<&'a StorytellerCredentialSet>,
}

/// List available splat models.
pub async fn omni_gen_list_splat_models(
  args: OmniGenListSplatModelsArgs<'_>,
) -> Result<OmniGenSplatModelsResponse, StorytellerError> {
  Ok(basic_json_get_request(
    args.api_host,
    OMNI_GEN_SPLAT_MODELS_PATH,
    args.maybe_creds,
  ).await?)
}

// ============================ Response types ============================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenSplatModelsResponse {
  #[serde(default)]
  pub success: bool,
  #[serde(default)]
  pub models: Vec<OmniGenSplatModelDetails>,
  #[serde(default)]
  pub providers: Vec<OmniGenSplatModelProviderDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenSplatModelProviderDetails {
  pub provider: ApiClientGenerationProvider,
  #[serde(default)]
  pub models: Vec<OmniGenSplatProviderModelDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenSplatProviderModelDetails {
  pub model: ApiClientSplatModel,
  #[serde(default)]
  pub overrides: Option<OmniGenSplatModelDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OmniGenSplatModelDetails {
  pub model: ApiClientSplatModel,
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
  pub image_references_supported: Option<bool>,
  #[serde(default)]
  pub image_references_max: Option<u16>,
  #[serde(default)]
  pub video_reference_supported: Option<bool>,
  #[serde(default)]
  pub panorama_supported: Option<bool>,
  #[serde(default)]
  pub disable_recaption_supported: Option<bool>,
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
          "model": "brand_new_splat_model",
          "model_creator": "some_new_studio",
          "surprise_new_field": 123
        }
      ],
      "providers": [
        { "provider": "new_provider", "models": [ { "model": "marble_1p1" } ] }
      ],
      "another_unexpected_top_level_field": "ignored"
    }"#;

    let resp: OmniGenSplatModelsResponse = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.models.len(), 1);
    assert_eq!(
      resp.models[0].model,
      ApiClientSplatModel::Unknown("brand_new_splat_model".to_string()),
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
    let resp: OmniGenSplatModelsResponse = serde_json::from_str("{}").unwrap();
    assert!(!resp.success);
    assert!(resp.models.is_empty());
    assert!(resp.providers.is_empty());
  }
}

/// Live tests that hit real servers. All `#[ignore]` so they never run by default.
/// Run explicitly, e.g.:
///   cargo test -p artcraft_client omni_gen_list_splat_models::live_tests -- --ignored --nocapture
#[cfg(test)]
mod live_tests {
  mod localhost {
    use super::super::{omni_gen_list_splat_models, OmniGenListSplatModelsArgs};
    use crate::utils::api_host::ApiHost;

    #[tokio::test]
    #[ignore] // live: hits http://localhost:12345
    async fn list_splat_models() {
      let api_host = ApiHost::Localhost { port: 12345 };
      let response = omni_gen_list_splat_models(OmniGenListSplatModelsArgs {
        api_host: &api_host,
        maybe_creds: None,
      })
      .await
      .expect("request should succeed");

      println!("[localhost] splat models response: {:#?}", response);
      assert!(response.success);
      assert!(!response.models.is_empty(), "expected at least one splat model");
    }
  }

  mod production {
    use super::super::{omni_gen_list_splat_models, OmniGenListSplatModelsArgs};
    use crate::utils::api_host::ApiHost;

    #[tokio::test]
    #[ignore] // live: hits https://api.storyteller.ai
    async fn list_splat_models() {
      let api_host = ApiHost::Storyteller;
      let response = omni_gen_list_splat_models(OmniGenListSplatModelsArgs {
        api_host: &api_host,
        maybe_creds: None,
      })
      .await
      .expect("request should succeed");

      println!("[production] splat models response: {:#?}", response);
      assert!(response.success);
      assert!(!response.models.is_empty(), "expected at least one splat model");
    }
  }
}
