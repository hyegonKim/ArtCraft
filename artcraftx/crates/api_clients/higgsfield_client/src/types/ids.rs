//! Opaque identifiers. Newtypes so a job id can't be handed to something
//! expecting a job set id; all are transparent strings on the wire.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

macro_rules! string_id {
  ($(#[$meta:meta])* $name:ident) => {
    $(#[$meta])*
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct $name(String);

    impl $name {
      pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
      }

      pub fn as_str(&self) -> &str {
        &self.0
      }

      pub fn into_string(self) -> String {
        self.0
      }
    }

    impl Display for $name {
      fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
      }
    }

    impl From<String> for $name {
      fn from(raw: String) -> Self {
        Self(raw)
      }
    }

    impl From<&str> for $name {
      fn from(raw: &str) -> Self {
        Self(raw.to_string())
      }
    }
  };
}

string_id! {
  /// A single generation job (one output). Poll it with the job status
  /// endpoints. UUID-shaped.
  JobId
}

string_id! {
  /// A job set: one enqueue request, which fans out into `batch_size` jobs.
  /// UUID-shaped.
  JobSetId
}

string_id! {
  /// The workspace (also used as the "project" id on job sets). UUID-shaped.
  WorkspaceId
}

string_id! {
  /// A Clerk user id, e.g. `user_2abc...`.
  UserId
}

string_id! {
  /// An uploaded reference file (image, video or audio), as returned by the
  /// media upload endpoints and referenced from generation requests.
  /// UUID-shaped.
  MediaId
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ids_are_transparent_strings() {
    let id = JobId::new("11111111-2222-4333-8444-555555555555");
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"11111111-2222-4333-8444-555555555555\"");
    let parsed: JobId = serde_json::from_str("\"abc\"").unwrap();
    assert_eq!(parsed.as_str(), "abc");
    assert_eq!(parsed.to_string(), "abc");
  }
}
