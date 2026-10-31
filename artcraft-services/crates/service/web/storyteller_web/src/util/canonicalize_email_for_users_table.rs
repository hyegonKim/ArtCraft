
/// This is to document the simple canonicalization routine we use currently
/// In the future we may want something more sophisticated that can handle
/// Gmail email address rules around period (.) and plus (+) as well as
/// broader internationalization support.
pub fn canonicalize_email_for_users_table(email: &str) -> String {
  email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
  use crate::util::canonicalize_email_for_users_table::canonicalize_email_for_users_table;

  #[test]
  fn lowercase() {
    assert_eq!(&canonicalize_email_for_users_table("FOO@BAR.COM"), "foo@bar.com");
    assert_eq!(&canonicalize_email_for_users_table("FOO@BAR.COM"), "foo@bar.com");
  }

  #[test]
  fn trim() {
    assert_eq!(&canonicalize_email_for_users_table("  foo@bar.com  "), "foo@bar.com");
  }

  #[test]
  fn cases_we_do_not_canonicalize() {
    // In the future, we may want to treat these differently:
    // 1) We may want to remove periods (.)
    // 2) We may want to scrub everything including and after a plus sign (+)
    assert_eq!(&canonicalize_email_for_users_table("bob.dole@gmail.com"), "bob.dole@gmail.com");
    assert_eq!(&canonicalize_email_for_users_table("zelda+1@gmail.com"), "zelda+1@gmail.com");
  }
}