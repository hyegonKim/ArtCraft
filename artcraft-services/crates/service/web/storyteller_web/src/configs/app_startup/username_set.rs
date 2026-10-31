use std::collections::{BTreeSet, HashSet};

#[derive(Clone)]
pub struct UsernameSet {
  /// "usernames" in our service are always lowercase.
  /// "display names" are case sensitive.
  usernames: HashSet<String>,
}

impl UsernameSet {
  pub fn from_comma_separated(comma_separated_list: &str) -> Self {
    let usernames = comma_separated_list.split(",")
        .into_iter()
        .map(|item| item.trim())
        .map(|item| item.to_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<HashSet<_>>();
    Self  {
      usernames
    }
  }

  pub fn len(&self) -> usize {
    self.usernames.len()
  }

  pub fn list_names(&self) -> BTreeSet<&str> {
    // NB: BTreeSet for sorting, which is good for logging at app startup
    self.usernames.iter().map(|item| item.as_str()).collect::<_>()
  }

  pub fn username_is_in_set(&self, username: &str) -> bool {
    // We lowercase the input username just in case a display name is passed
    let username = username.to_lowercase();
    self.usernames.contains(&username)
  }
}

#[cfg(test)]
mod tests {
  use crate::configs::app_startup::username_set::UsernameSet;

  fn build_set() -> UsernameSet {
    UsernameSet::from_comma_separated("bob,john,ALICE, BoB,jOhn,  ,,,  , aLiCe, joHn, bob,Alice")
  }

  #[test]
  fn test_deduplication() {
    let set = build_set();
    assert_eq!(set.len(), 3);
  }

  #[test]
  fn test_in_set() {
    let set = build_set();
    assert!(set.username_is_in_set("bob"));
    assert!(set.username_is_in_set("alice"));
    assert!(set.username_is_in_set("john"));
  }

  #[test]
  fn test_in_set_case_insensitive() {
    let set = build_set();

    assert!(set.username_is_in_set("Bob"));
    assert!(set.username_is_in_set("BOB"));

    assert!(set.username_is_in_set("AlIcE"));
    assert!(set.username_is_in_set("ALICE"));

    assert!(set.username_is_in_set("jOHN"));
    assert!(set.username_is_in_set("JOHN"));
  }

  #[test]
  fn test_not_in_set() {
    let set = build_set();

    // NB: The build_set() function might have attempted to introduce these,
    // but the struct constructor should filter blank spaces out
    assert!(!set.username_is_in_set(""));
    assert!(!set.username_is_in_set("    "));

    // Obviously not in set
    assert!(!set.username_is_in_set("mary"));
    assert!(!set.username_is_in_set("bill"));

    // These usernames are in the set, but not with padding
    assert!(!set.username_is_in_set("    alice   "));
    assert!(!set.username_is_in_set("bob  "));
    assert!(!set.username_is_in_set("  john"));
  }

  #[test]
  fn test_username_return() {
    let set = build_set();
    let usernames = set.list_names();
    assert_eq!(usernames.len(), 3);
    assert!(usernames.contains(&"alice"));
    assert!(usernames.contains(&"bob"));
    assert!(usernames.contains(&"john"));
    assert!(!usernames.contains(&"mary"));
  }
}
