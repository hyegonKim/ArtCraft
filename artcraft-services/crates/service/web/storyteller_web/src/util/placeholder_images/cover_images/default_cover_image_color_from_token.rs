use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokens::tokens::model_weights::ModelWeightToken;

/// We'll return an index from [0, NUMBER_OF_COLORS) - [inclusive, exclusive)
const NUMBER_OF_COLORS: u64 = 9;

/// Not that it matters, but this perturbs the hash.
const SALT_LIKE_OFFSET : u8 = 51;

/// We return an index instead of a filename, that way the frontend can drive.
/// The hash should be stable with respect to username.
pub fn default_cover_image_color_from_token(token: &ModelWeightToken) -> u8 {
  // Not as important as with usernames, but seems like a reasonable choice.
  let token_string = token.as_str().to_lowercase();

  let mut hasher = DefaultHasher::new();

  token_string.hash(&mut hasher);
  SALT_LIKE_OFFSET.hash(&mut hasher);

  let hash = hasher.finish();

  let cover_image_color_index = hash % NUMBER_OF_COLORS;
  cover_image_color_index as u8
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;
  use rand::distr::{Alphanumeric, SampleString};
  use tokens::tokens::model_weights::ModelWeightToken;

  use crate::util::placeholder_images::cover_images::default_cover_image_color_from_token::default_cover_image_color_from_token;

  #[test]
  fn test_stability() {
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("TOKEN")), 3);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("T_TOKEN")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("foo")), 6);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("bar")), 1);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("baz")), 6);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("bin")), 1);
  }

  #[test]
  fn test_case_insensitivity() {
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("T_TOKEN")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("T_TOKen")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("T_tOkEn")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("t_ToKeN")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("t_toKEN")), 7);
    assert_eq!(default_cover_image_color_from_token(&ModelWeightToken::new_from_str("t_token")), 7);
  }

  #[test]
  fn test_range() {
    let mut distribution = HashSet::new();
    for _ in 0..1000 {
      let random_token = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
      let random_token= ModelWeightToken::new_from_str(&random_token);
      let avatar_id = default_cover_image_color_from_token(&random_token);
      distribution.insert(avatar_id);
      assert!(avatar_id >= 0);
      assert!(avatar_id <= 8);
    }
    // NB: We could test frequency of the distribution too
    assert_eq!(distribution.len(), 9);
  }
}
