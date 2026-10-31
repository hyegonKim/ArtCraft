use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SimpleGenericJsonSuccess {
  pub success: bool,
}
