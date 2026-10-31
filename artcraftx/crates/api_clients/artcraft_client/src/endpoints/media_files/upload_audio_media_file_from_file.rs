use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::error::api_error::ApiError;
use crate::utils::api_host::ApiHost;
use crate::utils::constants::{APPLICATION_JSON, USER_AGENT};
use crate::utils::filter_bad_response::filter_bad_response;
use core_types::enums::generation_source::GenerationSource;
use uuid_utils::uuid::generate_random_uuid;
use log::debug;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_derive::Deserialize;
use std::path::Path;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use sqlite_identifiers::ids::prompt_token::PromptToken;

pub struct UploadAudioFromFileArgs<'a, P: AsRef<Path>> {
  pub api_host: &'a ApiHost,
  pub maybe_creds: Option<&'a StorytellerCredentialSet>,

  // NB: Path needs to be owned for the request.
  pub path: P,

  /// If provided, this is the prompt that this audio is associated with.
  pub maybe_prompt_token: Option<&'a PromptToken>,

  /// If provided, the third-party provider that generated this file.
  pub maybe_generation_provider: Option<GenerationSource>,
}

#[derive(Deserialize, Debug)]
pub struct UploadAudioMediaFileSuccessResponse {
  pub success: bool,
  pub media_file_token: MediaFileToken,
}

/// Upload an audio media file from a file.
pub async fn upload_audio_media_file_from_file<P: AsRef<Path>>(
  args: UploadAudioFromFileArgs<'_, P>,
) -> Result<UploadAudioMediaFileSuccessResponse, ApiError> {

  let url = get_route(args.api_host);

  debug!("Requesting {:?}", &url);

  let client = Client::builder()
      .gzip(true)
      .build()?;

  let file_bytes = std::fs::read(args.path.as_ref())?;
  let file_name = args.path.as_ref().file_name()
      .and_then(|n| n.to_str()).unwrap_or("file").to_string();
  let mut form = Form::new()
      .text("uuid_idempotency_token", generate_random_uuid())
      .part("file", Part::bytes(file_bytes).file_name(file_name));

  if let Some(prompt_token) = &args.maybe_prompt_token {
    form = form.text("maybe_prompt_token", prompt_token.to_string());
  }

  if let Some(provider) = &args.maybe_generation_provider {
    form = form.text("maybe_generation_provider", provider.to_str().to_string());
  }

  let mut request_builder = client.post(url)
      .header("User-Agent", USER_AGENT)
      .header("Accept", APPLICATION_JSON);

  if let Some(creds) = args.maybe_creds {
    if let Some(header) = &creds.maybe_as_cookie_header() {
      request_builder = request_builder.header("Cookie", header);
    }
  }

  let response = request_builder
      .multipart(form)
      .send()
      .await?;

  let response = filter_bad_response(response).await?;
  let response_body = &response.text().await?;

  let media_file = serde_json::from_str(&response_body)?;

  Ok(media_file)
}

fn get_route(api_host: &ApiHost) -> String {
  let api_hostname_and_scheme = api_host.to_api_hostname_and_scheme();
  format!("{}/v1/media_files/upload/audio", api_hostname_and_scheme)
}
