use crate::endpoints::jobs::job_status::JobStatusResponse;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::types::ids::JobId;
use crate::types::job_status::JobStatus;
use log::info;
use std::time::{Duration, Instant};

/// How to poll for a job to finish.
#[derive(Clone, Debug)]
pub struct WaitForJobOptions {
  /// Time between status checks.
  pub poll_interval: Duration,

  /// Give up after this long.
  pub timeout: Duration,
}

impl Default for WaitForJobOptions {
  fn default() -> Self {
    Self {
      poll_interval: Duration::from_secs(3),
      timeout: Duration::from_secs(10 * 60),
    }
  }
}

impl HiggsfieldSession {
  /// Poll until the job reaches a terminal state, then return its full
  /// status (with result URLs when it completed).
  ///
  /// Polls the cheap batch endpoint and only fetches the full record at the
  /// end. Errors with [`HiggsfieldClientError::JobTimedOut`] if the job
  /// isn't done within `options.timeout`, and
  /// [`HiggsfieldClientError::JobFailed`] if it ends in a non-success state.
  pub async fn wait_for_job(&self, job_id: &JobId, options: WaitForJobOptions) -> Result<JobStatusResponse, HiggsfieldError> {
    let started = Instant::now();
    let ids = [job_id.clone()];

    loop {
      let batch = self.job_status_batch(&ids).await?;

      if batch.missing.contains(job_id) {
        return Err(HiggsfieldClientError::JobNotFound(job_id.clone()).into());
      }

      let status = batch.find(job_id)
          .map(|item| item.status.clone())
          .unwrap_or_else(|| JobStatus::Other("<absent from batch response>".to_string()));

      info!("Higgsfield job {} status: {} ({}s elapsed)", job_id, status, started.elapsed().as_secs());

      if status.is_terminal() {
        let job = self.job_status(job_id).await?;
        if !job.status.is_success() {
          let maybe_reason = job.fail_reason().map(str::to_string);
          return Err(HiggsfieldClientError::JobFailed { job_id: job_id.clone(), status: job.status, maybe_reason }.into());
        }
        return Ok(job);
      }

      if started.elapsed() >= options.timeout {
        return Err(HiggsfieldClientError::JobTimedOut { job_id: job_id.clone(), last_status: status, waited: started.elapsed() }.into());
      }

      tokio::time::sleep(options.poll_interval).await;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn defaults_are_sane() {
    let options = WaitForJobOptions::default();
    assert!(options.poll_interval >= Duration::from_secs(1));
    assert!(options.timeout > options.poll_interval);
  }
}
