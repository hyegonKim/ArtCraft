use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// The frontend folks will tell us how many colors to use.
/// We'll return an index from [0, NUMBER_OF_COLORS) - [inclusive, exclusive)
const NUMBER_OF_COLORS : u64 = 9;

/// Not that it matters, but this perturbs the hash.
const SALT_LIKE_OFFSET : u8 = 51;

/// We return an index instead of a color, that way the frontend can drive.
/// The hash should be stable with respect to username.
pub fn default_avatar_color_from_username(username: &str) -> u8 {
  // We don't want to accidentally let case influence the output.
  // There's a potential of mixing up `username` and `display_name`.
  let username_lower = username.to_lowercase();

  // TODO: Maybe use consistent hashing so I don't have to keep updating the tests dramatically.
  let mut hasher = DefaultHasher::new();

  username_lower.hash(&mut hasher);
  SALT_LIKE_OFFSET.hash(&mut hasher);

  let hash = hasher.finish();

  let color_index = hash % NUMBER_OF_COLORS;
  color_index as u8
}

#[cfg(test)]
mod tests {
  use crate::http_server::common_responses::user_avatars::default_avatar_color_from_username::default_avatar_color_from_username;
  use rand::distr::{Alphanumeric, SampleString};
  use std::collections::HashSet;

  #[test]
  fn test_stability() {
    assert_eq!(default_avatar_color_from_username("echelon"), 3);
    assert_eq!(default_avatar_color_from_username("vegito"), 3);
    assert_eq!(default_avatar_color_from_username("bflat"), 6);
    assert_eq!(default_avatar_color_from_username("mechacosm"), 3);
    assert_eq!(default_avatar_color_from_username("sajattack"), 7);
    assert_eq!(default_avatar_color_from_username("zdisket"), 7);
  }

  #[test]
  fn test_case_insensitivity() {
    assert_eq!(default_avatar_color_from_username("echelon"), 3);
    assert_eq!(default_avatar_color_from_username("ECHELON"), 3);
    assert_eq!(default_avatar_color_from_username("EcHeLoN"), 3);
    assert_eq!(default_avatar_color_from_username("ECHElon"), 3);
    assert_eq!(default_avatar_color_from_username("echELON"), 3);
  }

  #[test]
  fn test_range() {
    let mut distribution = HashSet::new();
    for _ in 0..1000 {
      let random_username = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
      let avatar_id = default_avatar_color_from_username(&random_username);
      distribution.insert(avatar_id);
      assert!(avatar_id >= 0);
      assert!(avatar_id <= 9);
    }
    // NB: We could test frequency of the distribution too
    assert_eq!(distribution.len(), 9);
  }
}
