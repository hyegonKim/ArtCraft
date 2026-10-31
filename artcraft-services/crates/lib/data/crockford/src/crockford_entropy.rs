use rand::Rng;

use crate::{CROCKFORD_LOWERCASE_CHARSET, CROCKFORD_UPPERCASE_CHARSET};

// TODO: Modify these routines to filter out intra-string swear words.

/// Generate a crockford-encoded entropy string (uppercase)
pub fn crockford_entropy_upper(length: usize) -> String {
  crockford_entropy(length, CROCKFORD_UPPERCASE_CHARSET)
}

/// Generate a crockford-encoded entropy string (lowercase)
pub fn crockford_entropy_lower(length: usize) -> String {
  crockford_entropy(length, CROCKFORD_LOWERCASE_CHARSET)
}

#[inline]
fn crockford_entropy(length: usize, character_set: &[u8]) -> String {
  let mut rng = rand::thread_rng();

  (0..length).map(|_| {
    let idx = rng.gen_range(0..character_set.len());
    character_set[idx] as char
  }).collect()
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  use crate::crockford_entropy::{crockford_entropy_lower, crockford_entropy_upper};

  #[test]
  fn crockford_entropy_upper_length() {
    for len in 0..100 {
      assert_eq!(crockford_entropy_upper(len).len(), len);
    }
  }

  #[test]
  fn crockford_entropy_lower_length() {
    for len in 0..100 {
      assert_eq!(crockford_entropy_lower(len).len(), len);
    }
  }

  #[test]
  fn crockford_entropy_upper_randomness() {
    // NB: This *could* fail if the random pool or seed is bad and we generate duplicates.
    let mut set = HashSet::new();
    for _i in 0..100 {
      let entropy = crockford_entropy_upper(16);
      set.insert(entropy);
    }
    assert_eq!(set.len(), 100);
  }

  #[test]
  fn crockford_entropy_lower_randomness() {
    // NB: This *could* fail if the random pool or seed is bad and we generate duplicates.
    let mut set = HashSet::new();
    for _i in 0..100 {
      let entropy = crockford_entropy_lower(16);
      set.insert(entropy);
    }
    assert_eq!(set.len(), 100);
  }

  #[test]
  fn crockford_entropy_upper_uses_uppercase() {
    // NB: This *could* fail if the random pool or seed is bad and we get all numbers.
    for _i in 0..100 {
      let entropy = crockford_entropy_upper(16);
      assert_ne!(entropy, entropy.to_lowercase());
      assert_eq!(entropy, entropy.to_uppercase());
    }
  }

  #[test]
  fn crockford_entropy_upper_uses_lowercase() {
    // NB: This *could* fail if the random pool or seed is bad and we get all numbers.
    for _i in 0..100 {
      let entropy = crockford_entropy_lower(16);
      assert_ne!(entropy, entropy.to_uppercase());
      assert_eq!(entropy, entropy.to_lowercase());
    }
  }
}
