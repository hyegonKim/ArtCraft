use errors::AnyhowResult;

/// Create a bcrypt password hash.
/// TODO(bt,2023-10-05): Investigate stronger hashing (Argon2??)
/// TODO(bt,2023-10-05): Return something better than an AnyhowResult<()> !
pub fn bcrypt_password_hash(password: &str) -> AnyhowResult<String> {
  let password_hash = bcrypt_lib::hash(password, bcrypt_lib::DEFAULT_COST)?;
  Ok(password_hash)
}

#[cfg(test)]
mod tests {
  use crate::bcrypt::bcrypt_password_hash::bcrypt_password_hash;

  #[test]
  fn test_bcrypt_password_hashing() {
    let password = "password";
    for _ in 0..3 {
      // NB(1): Each bcrypt hash has an os-random salt
      // NB(2): Testing these takes time (as bcrypt is meant to), so we only test 3.
      let password_hash = bcrypt_password_hash(&password).unwrap();
      assert!(bcrypt_lib::verify(password, &password_hash).unwrap());
    }
  }
}
