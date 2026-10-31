use crockford::random_crockford_token;

/// Thread string identifiers
/// Used for leases, logging, etc.
/// This has nothing to do with actual OS thread names or IDs.
/// The `std::thread::ThreadId` type might be better for some cases.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ThreadId {
  id: String,
}

impl ThreadId {
  pub fn random_id() -> Self {
    Self {
      id: random_crockford_token(10),
    }
  }

  pub fn with_id(id: &str) -> Self {
    Self {
      id: id.to_string(),
    }
  }

  pub fn get_id(&self) -> &str {
    &self.id
  }
}

#[cfg(test)]
mod tests {
  use crate::payloads::thread_id::ThreadId;

  #[test]
  fn equals() {
    let a = ThreadId::with_id("foo");
    let b = ThreadId::with_id("foo");
    assert_eq!(a, b);
  }

  #[test]
  fn not_equals() {
    let a = ThreadId::with_id("foo");
    let b = ThreadId::with_id("bar");
    assert_ne!(a, b);
  }

  #[test]
  fn manual_id() {
    let t = ThreadId::with_id("foo");
    assert_eq!("foo", t.get_id());
  }

  #[test]
  fn random_id() {
    let t = ThreadId::random_id();
    assert_eq!(t.get_id().len(), 10);
  }
}