use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_splat_cost_and_generate_request::OmniGenSplatCostAndGenerateRequest;
use artcraft_client::credentials::api_or_web_creds::ApiOrWebCreds;
use artcraft_client::endpoints::omni_gen::generate::splat::omni_gen_splat::{omni_gen_splat_generate, OmniGenSplatGenerateArgs};

use crate::client::router_artcraft_client::RouterArtcraftClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::generate::generate_splat::generate_splat_response::{
  ArtcraftSplatResponsePayload, GenerateSplatResponse,
};

/// Send a splat generation request to the Artcraft omni-gen endpoint.
/// All Artcraft model request states delegate to this function.
pub async fn send_artcraft_omni_splat_request(
  request: &OmniGenSplatCostAndGenerateRequest,
  client: &RouterArtcraftClient,
) -> Result<GenerateSplatResponse, ArtcraftRouterError> {
  let api_or_web_creds = ApiOrWebCreds::from(&client.credentials);

  let response = omni_gen_splat_generate(OmniGenSplatGenerateArgs {
    api_host: &client.api_host,
    api_or_web_creds: Some(&api_or_web_creds),
    request,
  })
    .await
    .map_err(|err| ArtcraftRouterError::Provider(ProviderError::Storyteller(err)))?;

  Ok(GenerateSplatResponse::Artcraft(ArtcraftSplatResponsePayload {
    inference_job_token: response.inference_job_token,
    all_inference_job_tokens: response.all_job_tokens,
  }))
}
