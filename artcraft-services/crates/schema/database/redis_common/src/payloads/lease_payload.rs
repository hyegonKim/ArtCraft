use anyhow::anyhow;

use crockford::random_crockford_token;
use errors::AnyhowResult;

use crate::payloads::thread_id::ThreadId;

// TODO: Rename, document

// TODO: This should be JSON for future compat.
/// Lease values stored in Redis are in the form "{server_id}:{thread_id}"
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LeasePayload {
  pub server_id: String,
  pub thread_id: String,
}

impl LeasePayload {
  pub fn with_random_thread_id(server_id: &str) -> Self {
    let thread_id = random_crockford_token(10);
    Self::from_string_id(server_id, &thread_id)
  }

  pub fn from_thread_id(server_id: &str, thread_id: &ThreadId) -> Self {
    Self::from_string_id(server_id, thread_id.get_id())
  }

  pub fn from_string_id(server_id: &str, thread_id: &str) -> Self {
    Self {
      server_id: server_id.to_string(),
      thread_id: thread_id.to_string(),
    }
  }

  pub fn serialize(&self) -> String {
    format!("{}:{}", &self.server_id, &self.thread_id)
  }

  pub fn deserialize(payload: &str) -> AnyhowResult<Self> {
    let pieces = payload.split(":").collect::<Vec<_>>();
    if pieces.len() != 2 {
      return Err(anyhow!("Invalid payload: {}", payload));
    }

    let pair = (pieces.get(0), pieces.get(1));

    if let (Some(k), Some(v)) = pair {
      Ok(Self::from_string_id(k, v))
    } else {
      Err(anyhow!("Invalid payload: {}", payload))
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::payloads::lease_payload::LeasePayload;
  use crate::payloads::thread_id::ThreadId;

  #[test]
  fn equals() {
    let a = LeasePayload::from_string_id("foo", "bar");
    let b = LeasePayload::from_string_id("foo", "bar");
    assert_eq!(a, b);
    let t = ThreadId::with_id("bar");
    let b = LeasePayload::from_thread_id("foo", &t);
    assert_eq!(a, b);
  }

  #[test]
  fn not_equals() {
    let a = LeasePayload::from_string_id("foo", "bar");
    let b = LeasePayload::from_string_id("asdf", "qwerty");
    assert_ne!(a, b);
    let b = LeasePayload::from_string_id("foo", "ASDF");
    assert_ne!(a, b);
    let b = LeasePayload::from_string_id("ASDF", "bar");
    assert_ne!(a, b);
    let t = ThreadId::with_id("ASDF");
    let b = LeasePayload::from_thread_id("foo", &t);
    assert_ne!(a, b);
  }
}
