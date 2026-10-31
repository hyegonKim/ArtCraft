use crate::creds::credentials::Credentials;
use crate::error::api_error::ApiError;
use crate::error::classify_http_error_response::classify_http_error_response;
use crate::error::client_error::ClientError;
use crate::error::runwayml_error::RunwayMlError;
use log::{debug, error, warn};
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};

const GENERATE_URL: &str = "https://api.runwayml.com/v1/tasks";

// TODO: This endpoint is not implemented yet.
// TODO: This endpoint is not implemented yet.
// TODO: This endpoint is not implemented yet.

#[derive(Serialize)]
struct TaskRequest {
  /// eg. "gen4"
  #[serde(rename = "taskType")]
  task_type: String,
  /// eg. false
  internal: bool,
  /// inner request payload
  options: TaskRequestOptions,
  /// eg. 23129405
  #[serde(rename = "asTeamId")]
  as_team_id: u64,
  /// eg. fc66957b-2651-4362-aca0-5468e4ee61b0
  #[serde(rename = "sessionId")]
  session_id: String,
}

#[derive(Serialize)]
struct TaskRequestOptions {
  /// eg. Gen-4 3438043024
  name: String,
  /// eg. 3438043024
  seed: u64,
  /// Probably whether the request is free (exploreMode = unlimited queue)
  #[serde(rename = "exploreMode")]
  explore_mode: bool,
  /// eg. 5
  seconds: u8,
  /// eg. false
  watermark: bool,
  /// This can be blank / empty string!
  text_prompt: String,
  /// eg. "i2v"
  route: String,
  /// A URL, eg. https://d2jqrm6oza8nb6.cloudfront.net/datasets/586d77ff-b33b-4100-b4d3-c04f8b046b00.jpg?_jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJrZXlIYXNoIjoiNTJhMTY0N2RkZjFjMGU3NCIsImJ1Y2tldCI6InJ1bndheS1kYXRhc2V0cyIsInN0YWdlIjoicHJvZCIsImV4cCI6MTc0Nzc4NTYwMH0.N_TztqlU2pzPegOq7e8tn_6zb16OLmDmerlTtfr_-FI
  init_image: String,
  /// eg. 1280
  width: u16,
  /// eg. 720
  height: u16,
  /// eg. b5c42f29-d63d-4124-88c0-019ab1299829
  #[serde(rename = "assetGroupId")]
  asset_group_id: String,
}

pub async fn gen4_image_to_video(
  credentials: &Credentials,
) -> Result<(), RunwayMlError> {
  
  let jwt = match credentials.jwt_bearer_token.as_ref() {
    Some(token) => token,
    None => {
      error!("Failed to generate bearer token. No JWT bearer token found.");
      return Err(ClientError::NoJwtBearerToken.into());
    }
  };
  
  let endpoint_request = TaskRequest {
    task_type: "gen4".to_string(),
    internal: false,
    options: TaskRequestOptions {
      name: "Gen-4 3438043025".to_string(),
      seed: 3438043025,
      explore_mode: true,
      seconds: 5,
      watermark: false,
      text_prompt: "".to_string(),
      route: "i2v".to_string(),
      init_image: "https://d2jqrm6oza8nb6.cloudfront.net/datasets/586d77ff-b33b-4100-b4d3-c04f8b046b00.jpg?_jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJrZXlIYXNoIjoiNTJhMTY0N2RkZjFjMGU3NCIsImJ1Y2tldCI6InJ1bndheS1kYXRhc2V0cyIsInN0YWdlIjoicHJvZCIsImV4cCI6MTc0Nzc4NTYwMH0.N_TztqlU2pzPegOq7e8tn_6zb16OLmDmerlTtfr_-FI".to_string(),
      width: 1280,
      height: 720,
      asset_group_id: "b5c42f29-d63d-4124-88c0-019ab1299829".to_string(),
    },
    as_team_id: 23129405,
    session_id: "fc66957b-2651-4362-aca0-5468e4ee61b0".to_string(),
  };
  
  let client = Client::builder()
      .gzip(true)
      .build()
      .map_err(|err| {
        error!("Failed to create HTTP client: {}", err);
        RunwayMlError::Client(ClientError::ReqwestError(err))
      })?;
  
  let http_request = client.post(GENERATE_URL)
      .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:137.0) Gecko/20100101 Firefox/137.0")
      .header("Accept", "application/json")
      .header("Accept-Encoding", "gzip, deflate, br")
      .header("Accept-Language", "en-US,en;q=0.5")
      .header("Content-Type", "application/json")
      .header("Authorization", jwt.to_authorization_header_value());

  let http_request = http_request.json(&endpoint_request)
      .build()
      .map_err(|err| RunwayMlError::Client(ClientError::ReqwestError(err)))?;

  let response = client.execute(http_request)
      .await
      .map_err(|err| RunwayMlError::Api(ApiError::ReqwestError(err)))?;

  let status = response.status();

  if !status.is_success() {
    warn!("Gen4 image to video failure. Status: {:?}", status);
    
    let error = classify_http_error_response(response).await;
    return Err(error);
  }

  debug!("Gen4 I2V returned a 200.");
  
  let response_body = &response
      .text()
      .await
      .map_err(|err| RunwayMlError::Api(ApiError::ReqwestError(err)))?;

  debug!("Response: {}", response_body);

  //if &response_body == "null" {
  //  error!("Failed to generate bearer token. Response was `null`.");
  //  return Err(SoraError::FailedToGenerateBearer)
  //}

  //let auth_response: SoraAuthResponse = serde_json::from_str(&response_body)?;

  // TODO: This endpoint is not implemented yet.
  // TODO: This endpoint is not implemented yet.
  // TODO: This endpoint is not implemented yet.

  todo!("This is not yet implemented")
}
 
#[cfg(test)]
mod tests {
  use errors::AnyhowResult;

  #[tokio::test]
  #[ignore]
  async fn send_test_request() -> AnyhowResult<()> {
    Ok(())
  }
}