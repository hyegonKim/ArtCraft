use rand::Rng;

// Crockford characters
const CROCKFORD_CHARSET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[deprecated(note = "see internal 'crockford' and 'token' crates")]
pub fn random_crockford_token(length: usize) -> String {
  let mut rng = rand::thread_rng();

  let token: String = (0..length)
    .map(|_| {
      let idx = rng.gen_range(0..CROCKFORD_CHARSET.len());
      CROCKFORD_CHARSET[idx] as char
    })
    .collect();

  token
}

#[cfg(test)]
mod tests {
  use crate::random_crockford_token;

  #[test]
  fn random_token_length() {
    assert_eq!(random_crockford_token(0), "".to_string());
    assert_eq!(random_crockford_token(1).len(), 1);
    assert_eq!(random_crockford_token(10).len(), 10);
    assert_eq!(random_crockford_token(32).len(), 32);
  }
}
