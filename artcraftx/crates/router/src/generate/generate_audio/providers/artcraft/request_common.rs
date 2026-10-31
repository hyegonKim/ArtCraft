use artcraft_client::api_defs::omni_gen::cost_and_generate_requests::omni_gen_audio_cost_and_generate_request::OmniGenAudioCostAndGenerateRequest;
use artcraft_client::credentials::api_or_web_creds::ApiOrWebCreds;
use artcraft_client::endpoints::omni_gen::generate::audio::omni_gen_audio::{omni_gen_audio_generate, OmniGenAudioGenerateArgs};

use crate::client::router_artcraft_client::RouterArtcraftClient;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::generate::generate_audio::generate_audio_response::{
  ArtcraftAudioResponsePayload, GenerateAudioResponse,
};

/// Send an audio generation request to the Artcraft omni-gen endpoint.
/// All Artcraft model request states delegate to this function.
pub async fn send_artcraft_omni_audio_request(
  request: &OmniGenAudioCostAndGenerateRequest,
  client: &RouterArtcraftClient,
) -> Result<GenerateAudioResponse, ArtcraftRouterError> {
  let api_or_web_creds = ApiOrWebCreds::from(&client.credentials);

  let response = omni_gen_audio_generate(OmniGenAudioGenerateArgs {
    api_host: &client.api_host,
    api_or_web_creds: Some(&api_or_web_creds),
    request,
  })
    .await
    .map_err(|err| ArtcraftRouterError::Provider(ProviderError::Storyteller(err)))?;

  Ok(GenerateAudioResponse::Artcraft(ArtcraftAudioResponsePayload {
    inference_job_token: response.inference_job_token.clone(),
    all_inference_job_tokens: vec![response.inference_job_token],
  }))
}
