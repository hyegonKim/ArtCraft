//! Turn Higgsfield job failures into something a person can act on.
//!
//! Two audiences, two strings: `user_message` is what the frontend shows
//! (no job ids, no jargon, a next step where one exists) and `log_details`
//! is the full technical record (every job id, raw status, and the server's
//! `fail_reason`) for the logs and the tasks database.

use higgsfield_client::types::ids::JobId;
use higgsfield_client::types::job_status::JobStatus;
use sqlite_identifiers::enums::task_failure_type::TaskFailureType;

/// One job's failure, as observed while polling.
pub struct HiggsfieldJobFailure {
  pub job_id: JobId,
  pub kind: HiggsfieldJobFailureKind,
  /// The server's `meta.fail_reason`, when the full job record gave one.
  pub maybe_server_reason: Option<String>,
}

pub enum HiggsfieldJobFailureKind {
  /// The job ended in a non-success terminal status.
  TerminalStatus(JobStatus),
  /// The job claims success but returned no result URL to download.
  CompletedWithoutResult,
}

/// A whole task's failure, ready to persist and display.
pub struct HiggsfieldFailureReport {
  pub failure_type: TaskFailureType,
  /// Frontend-facing: friendly, actionable, no identifiers.
  pub user_message: String,
  /// Log/database-facing: every job id, raw status, and server reason.
  pub log_details: String,
}

impl HiggsfieldJobFailure {
  /// Higher wins when picking the message for a mixed batch: a stated
  /// server reason is the most specific thing we have, then the content
  /// filter (actionable), then the generic states.
  fn priority(&self) -> u8 {
    if self.maybe_server_reason.is_some() {
      return 6;
    }
    match &self.kind {
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Nsfw) => 5,
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed) => 4,
      HiggsfieldJobFailureKind::CompletedWithoutResult => 3,
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Cancelled) => 2,
      HiggsfieldJobFailureKind::TerminalStatus(_) => 1,
    }
  }

  fn user_message(&self) -> String {
    match &self.kind {
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Nsfw) => {
        "Higgsfield's content filter flagged this generation as NSFW. If your prompt and \
         references are innocuous this can be a false positive — try rewording the prompt \
         or swapping the reference media.".to_string()
      }
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed) => {
        match &self.maybe_server_reason {
          Some(reason) => format!("Higgsfield reported an error: {}", reason),
          None => "The generation failed on Higgsfield's side without a stated reason. \
                   This is often transient — try again.".to_string(),
        }
      }
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Cancelled) => {
        "The generation was cancelled on Higgsfield before it finished.".to_string()
      }
      HiggsfieldJobFailureKind::TerminalStatus(status) => {
        format!("The generation ended in an unexpected state on Higgsfield (\"{}\").", status)
      }
      HiggsfieldJobFailureKind::CompletedWithoutResult => {
        "Higgsfield reported the generation as complete but returned no output file. \
         Trying again usually works.".to_string()
      }
    }
  }

  fn failure_type(&self) -> TaskFailureType {
    match &self.kind {
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Nsfw) => TaskFailureType::RuleBansGeneratedContent,
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed) => TaskFailureType::GenerationFailed,
      HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Cancelled) => TaskFailureType::GenerationFailed,
      HiggsfieldJobFailureKind::TerminalStatus(_) => TaskFailureType::Unknown,
      HiggsfieldJobFailureKind::CompletedWithoutResult => TaskFailureType::GenerationFailed,
    }
  }

  fn log_detail(&self) -> String {
    let raw_state = match &self.kind {
      HiggsfieldJobFailureKind::TerminalStatus(status) => format!("status={}", status),
      HiggsfieldJobFailureKind::CompletedWithoutResult => "status=completed, no result URL".to_string(),
    };
    match &self.maybe_server_reason {
      Some(reason) => format!("job {}: {}, fail_reason={:?}", self.job_id, raw_state, reason),
      None => format!("job {}: {}", self.job_id, raw_state),
    }
  }
}

/// Summarize why a task (one or more Higgsfield jobs) failed.
/// `total_jobs` is the batch size, so partial context survives in the copy.
pub fn build_failure_report(failures: &[HiggsfieldJobFailure], total_jobs: usize) -> HiggsfieldFailureReport {
  let Some(primary) = failures.iter().max_by_key(|failure| failure.priority()) else {
    // Shouldn't happen (callers only report with >= 1 failure), but never
    // leave the user message empty.
    return HiggsfieldFailureReport {
      failure_type: TaskFailureType::Unknown,
      user_message: "The generation failed on Higgsfield for an unknown reason.".to_string(),
      log_details: "no job outcomes recorded".to_string(),
    };
  };

  let mut user_message = primary.user_message();
  if total_jobs > 1 {
    user_message = format!(
      "{} of {} generations in this batch failed. {}",
      failures.len(), total_jobs, user_message,
    );
  }

  let log_details = failures.iter()
      .map(HiggsfieldJobFailure::log_detail)
      .collect::<Vec<_>>()
      .join("; ");

  HiggsfieldFailureReport {
    failure_type: primary.failure_type(),
    user_message,
    log_details,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn failure(kind: HiggsfieldJobFailureKind, maybe_server_reason: Option<&str>) -> HiggsfieldJobFailure {
    HiggsfieldJobFailure {
      job_id: JobId::new("e82ac3d7-5282-4b71-be90-c7cfad846299"),
      kind,
      maybe_server_reason: maybe_server_reason.map(str::to_string),
    }
  }

  #[test]
  fn nsfw_maps_to_content_rule_with_actionable_copy() {
    let report = build_failure_report(
      &[failure(HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Nsfw), None)],
      1,
    );
    assert_eq!(report.failure_type, TaskFailureType::RuleBansGeneratedContent);
    assert!(report.user_message.contains("content filter"));
    assert!(report.user_message.contains("false positive"));
    // Job ids stay out of user copy, but land in the log details.
    assert!(!report.user_message.contains("e82ac3d7"));
    assert!(report.log_details.contains("e82ac3d7"));
    assert!(report.log_details.contains("status=nsfw"));
  }

  #[test]
  fn server_fail_reason_is_surfaced_verbatim() {
    let report = build_failure_report(
      &[failure(
        HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed),
        Some("Input audio duration is not supported. Please try a shorter audio."),
      )],
      1,
    );
    assert_eq!(report.failure_type, TaskFailureType::GenerationFailed);
    assert!(report.user_message.contains("Input audio duration is not supported"));
    assert!(report.log_details.contains("fail_reason=\"Input audio duration"));
  }

  #[test]
  fn failed_without_reason_reads_as_transient() {
    let report = build_failure_report(
      &[failure(HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed), None)],
      1,
    );
    assert!(report.user_message.contains("try again"));
  }

  #[test]
  fn unexpected_status_names_the_state() {
    let report = build_failure_report(
      &[failure(HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Other("mystery".into())), None)],
      1,
    );
    assert_eq!(report.failure_type, TaskFailureType::Unknown);
    assert!(report.user_message.contains("mystery"));
  }

  #[test]
  fn batch_failures_carry_counts_and_pick_the_most_specific_message() {
    let report = build_failure_report(
      &[
        failure(HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Cancelled), None),
        failure(
          HiggsfieldJobFailureKind::TerminalStatus(JobStatus::Failed),
          Some("Resolution not supported."),
        ),
      ],
      4,
    );
    assert!(report.user_message.starts_with("2 of 4 generations in this batch failed."));
    assert!(report.user_message.contains("Resolution not supported."));
    // Both jobs appear in the technical record.
    assert_eq!(report.log_details.matches("job ").count(), 2);
  }

  #[test]
  fn completed_without_result_is_its_own_story() {
    let report = build_failure_report(
      &[failure(HiggsfieldJobFailureKind::CompletedWithoutResult, None)],
      1,
    );
    assert_eq!(report.failure_type, TaskFailureType::GenerationFailed);
    assert!(report.user_message.contains("no output file"));
    assert!(report.log_details.contains("no result URL"));
  }
}
