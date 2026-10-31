use crate::api_defs::omni_gen::cost_and_generate_requests::omni_gen_audio_cost_and_generate_request::OmniGenAudioCostAndGenerateRequest;
use crate::api_defs::omni_gen::cost_response::omni_gen_audio_cost_response::OmniGenAudioCostResponse;
use crate::credentials::api_or_web_creds::ApiOrWebCreds;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::api_or_web_json_post_request::api_or_web_json_post_request;

pub const OMNI_GEN_AUDIO_COST_PATH: &str = "/v1/omni_gen/cost/audio";

pub struct OmniGenAudioCostArgs<'a> {
  pub api_host: &'a ApiHost,
  pub api_or_web_creds: Option<&'a ApiOrWebCreds>,
  pub request: &'a OmniGenAudioCostAndGenerateRequest,
}

/// Estimate the cost of a audio generation. Anonymous calls are allowed;
/// credentials personalize the estimate (free/unlimited plans).
pub async fn omni_gen_audio_cost(
  args: OmniGenAudioCostArgs<'_>,
) -> Result<OmniGenAudioCostResponse, StorytellerError> {
  Ok(api_or_web_json_post_request(
    args.api_host,
    OMNI_GEN_AUDIO_COST_PATH,
    args.api_or_web_creds,
    args.request,
  ).await?)
}
