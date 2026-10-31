use errors::AnyhowResult;

/// Any additional changes to the original file (such as resampling, format conversion)
/// are saved and persisted to the DB.
#[derive(Clone, Serialize, Deserialize)]
pub struct MediaUploadModificationDetails; // TODO: Inner fields

impl MediaUploadModificationDetails {

  pub fn from_json(json: &str) -> AnyhowResult<Self> {
    Ok(serde_json::from_str(json)?)
  }

  pub fn to_json(&self) -> AnyhowResult<String> {
    Ok(serde_json::to_string(self)?)
  }
}
