use crate::types::string_enum::string_enum;

string_enum! {
  /// Lifecycle state of a job.
  ///
  /// Observed: `waiting` (nano banana, right after enqueue), `queued` (GPT
  /// image, right after enqueue), `in_progress`, `completed`. The failure
  /// states haven't been captured yet, so they're best guesses; anything
  /// unexpected lands in `Other` rather than breaking status polling.
  JobStatus {
    Waiting => "waiting",
    Queued => "queued",
    InProgress => "in_progress",
    Completed => "completed",
    Failed => "failed",
    Cancelled => "cancelled",
    Nsfw => "nsfw",
  }
}

impl JobStatus {
  /// The job has reached an end state (successfully or not).
  pub fn is_terminal(&self) -> bool {
    match self {
      Self::Waiting | Self::Queued | Self::InProgress => false,
      Self::Completed | Self::Failed | Self::Cancelled | Self::Nsfw => true,
      // Unknown states: don't claim to know. Callers polling should keep
      // going and rely on their own timeout.
      Self::Other(_) => false,
    }
  }

  pub fn is_success(&self) -> bool {
    matches!(self, Self::Completed)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in JobStatus::known_variants() {
      assert_eq!(&JobStatus::from_str_lossy(variant.as_str()), variant);
    }
  }

  #[test]
  fn terminal_states() {
    assert!(!JobStatus::Waiting.is_terminal());
    assert!(!JobStatus::Queued.is_terminal());
    assert!(!JobStatus::InProgress.is_terminal());
    assert!(JobStatus::Completed.is_terminal());
    assert!(JobStatus::Completed.is_success());
    assert!(JobStatus::Failed.is_terminal());
    assert!(!JobStatus::Failed.is_success());
    assert!(!JobStatus::Other("mystery".into()).is_terminal());
  }
}
