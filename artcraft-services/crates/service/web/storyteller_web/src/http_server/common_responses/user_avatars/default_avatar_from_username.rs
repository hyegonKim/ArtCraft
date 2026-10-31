use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// There are currently 25 avatars numbered 0 to 24 (0-indexed).
/// The original dataset was numbered 1 - 25, but I renamed 25 to 0.
const NUMBER_OF_AVATARS : u64 = 25;

/// Not that it matters, but this perturbs the hash.
const SALT_LIKE_OFFSET : u8 = 21;

/// We return an index instead of a filename, that way the frontend can drive.
/// The hash should be stable with respect to username.
pub fn default_avatar_from_username(username: &str) -> u8 {
  // We don't want to accidentally let case influence the output.
  // There's a potential of mixing up `username` and `display_name`.
  let username_lower = username.to_lowercase();

  let mut hasher = DefaultHasher::new();

  username_lower.hash(&mut hasher);
  SALT_LIKE_OFFSET.hash(&mut hasher);

  let hash = hasher.finish();

  let avatar_index= hash % NUMBER_OF_AVATARS;
  avatar_index as u8
}

#[cfg(test)]
mod tests {
  use crate::http_server::common_responses::user_avatars::default_avatar_from_username::default_avatar_from_username;
  use rand::distr::{Alphanumeric, SampleString};
  use std::collections::HashSet;

  #[test]
  fn test_stability() {
    assert_eq!(default_avatar_from_username("echelon"), 10);
    assert_eq!(default_avatar_from_username("vegito"), 20);
    assert_eq!(default_avatar_from_username("bflat"), 22);
    assert_eq!(default_avatar_from_username("mechacosm"), 15);
    assert_eq!(default_avatar_from_username("sajattack"), 15);
    assert_eq!(default_avatar_from_username("zdisket"), 19);
  }

  #[test]
  fn test_case_insensitivity() {
    assert_eq!(default_avatar_from_username("echelon"), 10);
    assert_eq!(default_avatar_from_username("ECHELON"), 10);
    assert_eq!(default_avatar_from_username("EcHeLoN"), 10);
    assert_eq!(default_avatar_from_username("ECHElon"), 10);
    assert_eq!(default_avatar_from_username("echELON"), 10);
  }

  #[test]
  fn test_range() {
    let mut distribution = HashSet::new();
    for _ in 0..1000 {
      let random_username = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
      let avatar_id = default_avatar_from_username(&random_username);
      distribution.insert(avatar_id);
      assert!(avatar_id >= 0);
      assert!(avatar_id <= 24);
    }
    // NB: We could test frequency of the distribution too
    assert_eq!(distribution.len(), 25);
  }
}
