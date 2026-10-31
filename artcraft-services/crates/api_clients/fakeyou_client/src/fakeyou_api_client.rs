use std::sync::Arc;
use std::time::Duration;

use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};
use backoff::future::retry;
use errors::{anyhow, AnyhowError, AnyhowResult};
use log::{error, warn};
use reqwest::{Client, ClientBuilder, RequestBuilder, StatusCode, Url};
use reqwest::cookie::Jar;

use crate::api::tts_inference::{CreateTtsInferenceRequest, CreateTtsInferenceResponse, TtsInferenceJobStatus};
use crate::credentials::FakeYouCredentials;

const AUTHORIZATION_HEADER: &str = "Authorization";
const SESSION_COOKIE_NAME : &str = "session";

/// A client to consume FakeYou's API via API sessions (vs user sessions).
pub struct FakeYouApiClient {
  api_domain: String,
  credentials: FakeYouCredentials,
  client: Client,
}

impl FakeYouApiClient {

  pub fn make_production_client_from_credentials(credentials: FakeYouCredentials) -> AnyhowResult<Self> {
    let mut client_builder = ClientBuilder::new();

    match &credentials {
      FakeYouCredentials::SessionCookie(credentials ) => {
        let cookie = format!(
          "{}={}; Domain=.fakeyou.com",
          SESSION_COOKIE_NAME,
          &credentials.cookie_value
        );
        let url = "https://fakeyou.com".parse::<Url>()?;
        let jar = Jar::default();
        jar.add_cookie_str(&cookie, &url);
        client_builder = client_builder
            .cookie_store(true)
            .cookie_provider(Arc::new(jar));
      }
      _ => {}, // NB: Other methods don't need handling.
    }

    Ok(Self {
      api_domain: "api.fakeyou.com".to_string(),
      credentials,
      client: client_builder.build()?,
    })
  }

  pub fn make_production_client_from_api_token(api_token: &str) -> Self {
    let credentials = FakeYouCredentials::from_api_token(api_token);
    Self {
      api_domain: "api.fakeyou.com".to_string(),
      credentials,
      client: Client::new(),
    }
  }

  // TODO: need to yield better, more "library"-appropriate errors.
  pub async fn create_tts_inference(&self, request: CreateTtsInferenceRequest<'_>) -> AnyhowResult<CreateTtsInferenceResponse> {
    let response = self.do_create_tts_inference(&request).await?;
    let response = serde_json::from_str(&response)?;
    Ok(response)
  }

  pub async fn create_tts_inference_with_backoff(&self, request: CreateTtsInferenceRequest<'_>) -> AnyhowResult<CreateTtsInferenceResponse> {
    //let backoff = ExponentialBackoff::default();
    let mut backoff = ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_secs(3)) // Initial retry interval: 3 seconds
        .with_randomization_factor(0.5) // 50% below, 50% above interval
        .with_multiplier(2.0) // Double the wait
        .with_max_elapsed_time(Some(Duration::from_secs(60)))
        //.with_max_interval(Duration::from_secs(30))
        .build();

    let response = retry(backoff, || async {
      Ok(self.do_create_tts_inference(&request).await
          .map_err(|err| {
            //warn!("reqwest err: {:?}", err);
            err
          })?
      )
    }).await?;

    let response = serde_json::from_str(&response)?;
    Ok(response)
  }

  pub async fn do_create_tts_inference(&self, request: &CreateTtsInferenceRequest<'_>) -> Result<String, AnyhowError> {
    let url = format!("https://{}/tts/inference", self.api_domain);
    let response = self.add_credentials(self.client
        .post(url))
        .json(request)
        .send()
        .await?;

    match response.status().as_u16() {
      200 => { /* Okay */ }
      _ => return Err(anyhow!("bad status: {}", response.status())),
    };

    let response = response.text().await?;
    Ok(response)
  }

  // TODO: need to yield better, more "library"-appropriate errors.
  pub async fn get_tts_inference_job_status(&self, inference_job_token: &str) -> AnyhowResult<TtsInferenceJobStatus> {
    let url = format!("https://{}/tts/job/{}", self.api_domain, inference_job_token);

    let response = self.add_credentials(self.client
        .get(url))
        .send()
        .await?
        .text()
        .await?;

    let response = serde_json::from_str(&response)?;

    Ok(response)
  }

  fn add_credentials(&self, request_builder: RequestBuilder) -> RequestBuilder {
    match &self.credentials {
      FakeYouCredentials::ApiToken(api_token) => {
        request_builder.header(AUTHORIZATION_HEADER, &api_token.token)
      }
      _ => request_builder, // NB: Other auth types already handled.
    }
  }
}
