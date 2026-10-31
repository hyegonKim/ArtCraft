use md5::{Digest, Md5};

/// See https://gravatar.com/
pub fn email_to_gravatar_hash(email_address: &str) -> String {
  let email = email_address.trim().to_lowercase();

  let mut hasher = Md5::new();
  hasher.update(email);
  let hash = hasher.finalize();
  let gravatar_hash = format!("{:x}", hash);

  gravatar_hash
}

#[cfg(test)]
mod tests {
  use crate::md5::email_to_gravatar_hash::email_to_gravatar_hash;

  #[test]
  fn test_email_hashing() {
    assert_eq!(email_to_gravatar_hash("email@email.com"), "4f64c9f81bb0d4ee969aaf7b4a5a6f40".to_string());
    assert_eq!(email_to_gravatar_hash("foo@bar.com"), "f3ada405ce890b6f8204094deb12d8a8".to_string());
    assert_eq!(email_to_gravatar_hash("brandon@storyteller.ai"), "54bd9d9491cd63f733f0970ae4059438".to_string());
  }
}
