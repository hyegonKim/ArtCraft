
// TODO: Rename, document

use errors::AnyhowResult;

#[derive(Serialize, Deserialize, Debug)]
pub struct ObsActivePayload {
  pub twitch_user_id: String,
}

impl ObsActivePayload {
  pub fn new(twitch_user_id: &str) -> Self {
    Self {
      twitch_user_id: twitch_user_id.to_string(),
    }
  }

  pub fn from_json_str(json: &str) -> AnyhowResult<Self> {
    Ok(serde_json::from_str(json)?)
  }

  pub fn serialize(&self) -> AnyhowResult<String> {
    Ok(serde_json::to_string(&self)?)
  }
}