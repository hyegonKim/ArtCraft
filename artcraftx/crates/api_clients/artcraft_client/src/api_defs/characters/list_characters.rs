use serde_derive::{Deserialize, Serialize};

use crate::api_defs::common::responses::media_links::MediaLinks;
use crate::enums::common::generation::common_model_type::CommonModelType;
use crate::tokens::characters::CharacterToken;

/// Query string parameters for listing characters.
#[derive(Deserialize)]
pub struct ListCharactersQuery {
  /// Optional cursor for pagination.
  pub cursor: Option<u64>,
}

/// Response body for listing characters in the current session.
#[derive(Serialize)]
pub struct ListCharactersResponse {
  pub success: bool,
  pub characters: Vec<ListCharactersEntry>,
  pub next_cursor: Option<u64>,
}

/// A character entry in the list response.
#[derive(Serialize)]
pub struct ListCharactersEntry {
  pub token: CharacterToken,

  /// Which models this character can be used with.
  pub models: Vec<CommonModelType>,

  pub name: String,

  pub maybe_description: Option<String>,

  pub maybe_avatar: Option<MediaLinks>,

  pub maybe_full_image: Option<MediaLinks>,
}
