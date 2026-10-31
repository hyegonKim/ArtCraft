use serde_derive::{Deserialize, Serialize};

use crate::tokens::characters::CharacterToken;

/// Path parameters for deleting a character.
#[derive(Deserialize)]
pub struct DeleteCharacterPathInfo {
  pub character_token: CharacterToken,
}

/// Response body for deleting a character.
#[derive(Serialize)]
pub struct DeleteCharacterResponse {
  pub success: bool,
}
