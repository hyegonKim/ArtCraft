use crate::api_defs::omni_gen::cost_and_generate_requests::omni_gen_video_cost_and_generate_request::OmniGenVideoCostAndGenerateRequest;
use crate::api_defs::omni_gen::generate_response::omni_gen_video_generate_response::OmniGenVideoGenerateResponse;
use crate::credentials::api_or_web_creds::ApiOrWebCreds;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::api_or_web_json_post_request::api_or_web_json_post_request;

pub const OMNI_GEN_VIDEO_GENERATE_PATH: &str = "/v1/omni_gen/generate/video";

pub struct OmniGenVideoGenerateArgs<'a> {
  pub api_host: &'a ApiHost,
  pub api_or_web_creds: Option<&'a ApiOrWebCreds>,
  pub request: &'a OmniGenVideoCostAndGenerateRequest,
}

pub async fn omni_gen_video_generate(
  args: OmniGenVideoGenerateArgs<'_>,
) -> Result<OmniGenVideoGenerateResponse, StorytellerError> {
  Ok(api_or_web_json_post_request(
    args.api_host,
    OMNI_GEN_VIDEO_GENERATE_PATH,
    args.api_or_web_creds,
    args.request,
  ).await?)
}
