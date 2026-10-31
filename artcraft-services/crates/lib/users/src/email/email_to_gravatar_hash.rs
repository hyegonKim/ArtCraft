use md5::{Digest, Md5};

pub fn email_to_gravatar_hash(email_address: &str) -> String {
  let email = email_address.trim().to_lowercase();

  let mut hasher = Md5::new();
  hasher.update(email);
  let hash = hasher.finalize();
  let gravatar_hash = format!("{:x}", hash);

  gravatar_hash
}

#[cfg(test)]
pub mod tests {
  use crate::email::email_to_gravatar_hash::email_to_gravatar_hash;

  #[test]
  fn test_gravatar() {
    // Email 1
    assert_eq!(email_to_gravatar_hash("example@example.com"), "23463b99b62a72f26ed677cc556c44e8".to_string());
    assert_eq!(email_to_gravatar_hash("EXAMPLE@EXAMPLE.COM"), "23463b99b62a72f26ed677cc556c44e8".to_string());
    assert_eq!(email_to_gravatar_hash("  example@example.com \n "), "23463b99b62a72f26ed677cc556c44e8".to_string());

    // Email 2
    assert_eq!(email_to_gravatar_hash("echelon@gmail.com"), "37e7e18f19d8f654a1ace405c846a9de".to_string());
    assert_eq!(email_to_gravatar_hash("ECHELON@GMAIL.COM"), "37e7e18f19d8f654a1ace405c846a9de".to_string());
    assert_eq!(email_to_gravatar_hash("  ECHELON@GMAIL.COM  "), "37e7e18f19d8f654a1ace405c846a9de".to_string());

    // Misc
    assert_eq!(email_to_gravatar_hash(""), "d41d8cd98f00b204e9800998ecf8427e".to_string());
    assert_eq!(email_to_gravatar_hash("   "), "d41d8cd98f00b204e9800998ecf8427e".to_string());
  }
}