use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokens::tokens::model_weights::ModelWeightToken;

/// There are currently 25 cover images numbered 0 to 24 (0-indexed).
/// The original dataset was numbered 1 - 25, but I renamed 25 to 0.
const NUMBER_OF_COVER_IMAGES: u64 = 25;

/// Not that it matters, but this perturbs the hash.
const SALT_LIKE_OFFSET : u8 = 21;

/// We return an index instead of a filename, that way the frontend can drive.
/// The hash should be stable with respect to username.
pub fn default_cover_image_from_token(token: &ModelWeightToken) -> u8 {
  // Not as important as with usernames, but seems like a reasonable choice.
  let token_string = token.as_str().to_lowercase();

  let mut hasher = DefaultHasher::new();

  token_string.hash(&mut hasher);
  SALT_LIKE_OFFSET.hash(&mut hasher);

  let hash = hasher.finish();

  let cover_image_index = hash % NUMBER_OF_COVER_IMAGES;
  cover_image_index as u8
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;
  use rand::distr::{Alphanumeric, SampleString};
  use tokens::tokens::model_weights::ModelWeightToken;

  use crate::util::placeholder_images::cover_images::default_cover_image_from_token::default_cover_image_from_token;

  #[test]
  fn test_stability() {
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("TOKEN")), 0);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("T_TOKEN")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("foo")), 1);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("bar")), 16);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("baz")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("bin")), 21);
  }

  #[test]
  fn test_case_insensitivity() {
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("T_TOKEN")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("T_TOKen")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("T_tOkEn")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("t_ToKeN")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("t_toKEN")), 5);
    assert_eq!(default_cover_image_from_token(&ModelWeightToken::new_from_str("t_token")), 5);
  }

  #[test]
  fn test_range() {
    let mut distribution = HashSet::new();
    for _ in 0..1000 {
      let random_token = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
      let random_token= ModelWeightToken::new_from_str(&random_token);
      let avatar_id = default_cover_image_from_token(&random_token);
      distribution.insert(avatar_id);
      assert!(avatar_id >= 0);
      assert!(avatar_id <= 24);
    }
    // NB: We could test frequency of the distribution too
    assert_eq!(distribution.len(), 25);
  }
}
