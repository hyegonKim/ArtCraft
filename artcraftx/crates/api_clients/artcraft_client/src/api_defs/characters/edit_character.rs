use serde_derive::{Deserialize, Serialize};

use crate::tokens::characters::CharacterToken;

/// Request body for editing a character.
#[derive(Deserialize)]
pub struct EditCharacterRequest {
  /// The character to edit.
  pub token: CharacterToken,

  /// Updated name (if changing).
  pub updated_name: Option<String>,

  /// Updated description (if changing).
  pub updated_description: Option<String>,
}

/// Response body for editing a character.
#[derive(Serialize)]
pub struct EditCharacterResponse {
  pub success: bool,
}
