use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_mesh_cost_and_generate_request::OmniGenMeshCostAndGenerateRequest;
use artcraft_client::credentials::api_or_web_creds::ApiOrWebCreds;
use artcraft_client::endpoints::omni_gen::generate::mesh::omni_gen_mesh::{omni_gen_mesh_generate, OmniGenMeshGenerateArgs};

use crate::client::router_artcraft_client::RouterArtcraftClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::generate::generate_mesh::generate_mesh_response::{
  ArtcraftMeshResponsePayload, GenerateMeshResponse,
};

/// Send a mesh generation request to the Artcraft omni-gen endpoint.
/// All Artcraft model request states delegate to this function.
pub async fn send_artcraft_omni_mesh_request(
  request: &OmniGenMeshCostAndGenerateRequest,
  client: &RouterArtcraftClient,
) -> Result<GenerateMeshResponse, ArtcraftRouterError> {
  let api_or_web_creds = ApiOrWebCreds::from(&client.credentials);

  let response = omni_gen_mesh_generate(OmniGenMeshGenerateArgs {
    api_host: &client.api_host,
    api_or_web_creds: Some(&api_or_web_creds),
    request,
  })
    .await
    .map_err(|err| ArtcraftRouterError::Provider(ProviderError::Storyteller(err)))?;

  Ok(GenerateMeshResponse::Artcraft(ArtcraftMeshResponsePayload {
    inference_job_token: response.inference_job_token,
    all_inference_job_tokens: response.all_job_tokens,
  }))
}
