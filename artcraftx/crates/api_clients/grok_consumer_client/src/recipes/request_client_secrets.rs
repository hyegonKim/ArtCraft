use crate::credentials::grok_client_secrets::GrokClientSecrets;
use crate::credentials::grok_cookies::GrokCookies;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use crate::endpoint_bindings::old_bindings::index_page::get_index_page::{get_index, GetIndexPageArgs};
use crate::endpoint_bindings::old_bindings::index_page::parsers::index::parse_index_baggage::parse_index_baggage;
use crate::endpoint_bindings::old_bindings::index_page::parsers::index::parse_index_sentry_trace::parse_index_sentry_trace;
use crate::endpoint_bindings::old_bindings::index_page::parsers::index::parse_index_user_email::parse_index_user_email;
use crate::endpoint_bindings::old_bindings::index_page::parsers::index::parse_index_user_id::parse_index_user_id;

pub struct RequestClientSecretsArgs<'a> {
  pub cookies: &'a GrokCookies,
}

/// Load the per-session secrets Grok's web app reads out of `index.html`:
/// tracing headers (`baggage`, `sentry_trace`) and the signed-in user's
/// id/email. The `x-statsig-id` request signature is *not* derived here — it is
/// minted by a real browser (see the `grok_consumer_statsig` crate) and passed
/// through [`GrokRequestHeaders`](crate::credentials::grok_request_headers).
pub async fn request_client_secrets(args: RequestClientSecretsArgs<'_>) -> Result<GrokClientSecrets, GrokError> {
  let index = get_index(GetIndexPageArgs {
    cookie: args.cookies.as_str(),
  }).await?;
  let html = &index.body;

  let baggage = parse_index_baggage(html)
      .ok_or_else(|| GrokGenericApiError::IndexHtmlDidNotIncludeExpectedData {
        message: "Index did not include baggage.".to_string()
      })?;

  let sentry_trace = parse_index_sentry_trace(html)
      .ok_or_else(|| GrokGenericApiError::IndexHtmlDidNotIncludeExpectedData {
        message: "Index did not include sentry trace.".to_string()
      })?;

  let user_id = parse_index_user_id(html)
      .ok_or_else(|| GrokGenericApiError::IndexHtmlDidNotIncludeExpectedData {
        message: "Index did not include user id.".to_string()
      })?;

  // NB: Optional.
  let maybe_user_email = parse_index_user_email(html);

  Ok(GrokClientSecrets {
    baggage,
    sentry_trace,
    user_id,
    user_email: maybe_user_email,
  })
}

#[cfg(test)]
mod tests {
  use crate::recipes::request_client_secrets::{request_client_secrets, RequestClientSecretsArgs};
  use crate::test_utils::grok_test_secrets::load_grok_test_secrets;
  use errors::AnyhowResult;

  #[tokio::test]
  #[ignore] // Manual test invocation
  async fn test() -> AnyhowResult<()> {
    let cookies = load_grok_test_secrets()?.cookies;

    let secrets = request_client_secrets(RequestClientSecretsArgs {
      cookies: &cookies,
    }).await?;

    println!("Baggage: {:?}", secrets.baggage);
    println!("Sentry trace: {:?}", secrets.sentry_trace);
    println!("User ID: {:?}", secrets.user_id);
    println!("User Email: {:?}", secrets.user_email);

    assert_eq!(1, 2);
    Ok(())
  }
}
