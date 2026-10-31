use crate::api_defs::omni_gen::cost_and_generate_requests::omni_gen_audio_cost_and_generate_request::OmniGenAudioCostAndGenerateRequest;
use crate::api_defs::omni_gen::generate_response::omni_gen_audio_generate_response::OmniGenAudioGenerateResponse;
use crate::credentials::api_or_web_creds::ApiOrWebCreds;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::api_or_web_json_post_request::api_or_web_json_post_request;

pub const OMNI_GEN_AUDIO_GENERATE_PATH: &str = "/v1/omni_gen/generate/audio";

pub struct OmniGenAudioGenerateArgs<'a> {
  pub api_host: &'a ApiHost,
  pub api_or_web_creds: Option<&'a ApiOrWebCreds>,
  pub request: &'a OmniGenAudioCostAndGenerateRequest,
}

pub async fn omni_gen_audio_generate(
  args: OmniGenAudioGenerateArgs<'_>,
) -> Result<OmniGenAudioGenerateResponse, StorytellerError> {
  Ok(api_or_web_json_post_request(
    args.api_host,
    OMNI_GEN_AUDIO_GENERATE_PATH,
    args.api_or_web_creds,
    args.request,
  ).await?)
}
