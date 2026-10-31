use crate::credentials::api_or_web_creds::ApiOrWebCreds;
use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::error::storyteller_error::StorytellerError;
use crate::utils::api_host::ApiHost;
use crate::utils::api_or_web_json_post_request::api_or_web_json_post_request;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// JSON POST authenticated by web session cookies only. Thin wrapper over
/// [`api_or_web_json_post_request`] for endpoints not yet migrated to
/// [`ApiOrWebCreds`].
pub async fn basic_json_post_request<Req: Serialize, Res: DeserializeOwned>(
  api_host: &ApiHost,
  route_path: &str,
  maybe_creds: Option<&StorytellerCredentialSet>,
  request: Req,
) -> Result<Res, StorytellerError> {
  let maybe_api_or_web_creds = maybe_creds.map(ApiOrWebCreds::from);

  api_or_web_json_post_request(
    api_host,
    route_path,
    maybe_api_or_web_creds.as_ref(),
    &request,
  ).await
}
