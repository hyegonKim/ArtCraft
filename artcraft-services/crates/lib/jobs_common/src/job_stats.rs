use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::anyhow;
use chrono::{DateTime, Utc};

use errors::AnyhowResult;

/// Job stats uses interior mutability to be easy to copy around.
#[derive(Clone)]
pub struct JobStats {
   inner: Arc<RwLock<JobStatsInner>>,
}

/// Private inner implementation that may grow additional stats in the future.
#[derive(Default)]
struct JobStatsInner {
   pub total_success_count: u64,
   pub total_failure_count: u64,
   pub consecutive_success_count: u64,
   pub consecutive_failure_count: u64,

   pub maybe_current_job: Option<JobDetailInner>,
}

#[derive(Default)]
struct JobDetailInner {
   pub job_token: String,
   pub job_started_at: DateTime<Utc>,
}

/// Public result type.
#[derive(Default, Debug, Clone)]
pub struct SuccessAndFailureStats {
   pub total_success_count: u64,
   pub total_failure_count: u64,
   pub consecutive_success_count: u64,
   pub consecutive_failure_count: u64,

   pub maybe_current_job: Option<JobDetail>,
}


/// Public result type.
#[derive(Default, Debug, Clone)]
pub struct JobDetail {
   pub job_token: String,
   pub job_started_at: DateTime<Utc>,
}

impl JobStats {
   pub fn new() -> Self {
      Self {
         inner: Arc::new(RwLock::new(JobStatsInner::default())),
      }
   }

   pub fn get_status(&self) -> AnyhowResult<SuccessAndFailureStats> {
      // NB: lock errors can't be moved between threads, so we change their type
      let lock = self.inner.read()
          .map_err(|e| anyhow!("lock read error: {:?}", e))?;

      Ok(SuccessAndFailureStats {
         total_success_count: lock.total_success_count,
         total_failure_count: lock.total_failure_count,
         consecutive_success_count: lock.consecutive_success_count,
         consecutive_failure_count: lock.consecutive_failure_count,
         maybe_current_job: lock.maybe_current_job.as_ref().map(|job| {
            JobDetail {
               job_token: job.job_token.clone(),
               job_started_at: job.job_started_at,
            }
         }),
      })
   }

   pub fn increment_failure_count(&self) -> AnyhowResult<SuccessAndFailureStats> {
      // NB: lock errors can't be moved between threads, so we change their type
      let mut lock = self.inner.write()
          .map_err(|e| anyhow!("lock error: {:?}", e))?;

      lock.total_failure_count = lock.total_failure_count.saturating_add(1);
      lock.consecutive_success_count = 0;
      lock.consecutive_failure_count = lock.consecutive_failure_count.saturating_add(1);

      Ok(SuccessAndFailureStats {
         total_success_count: lock.total_success_count,
         total_failure_count: lock.total_failure_count,
         consecutive_success_count: lock.consecutive_success_count,
         consecutive_failure_count: lock.consecutive_failure_count,
         maybe_current_job: lock.maybe_current_job.as_ref().map(|job| {
            JobDetail {
               job_token: job.job_token.clone(),
               job_started_at: job.job_started_at,
            }
         }),
      })
   }

   pub fn increment_success_count(&self) -> AnyhowResult<SuccessAndFailureStats> {
      // NB: lock errors can't be moved between threads, so we change their type
      let mut lock = self.inner.write()
          .map_err(|e| anyhow!("lock error: {:?}", e))?;

      lock.total_success_count = lock.total_success_count.saturating_add(1);
      lock.consecutive_success_count = lock.consecutive_success_count.saturating_add(1);
      lock.consecutive_failure_count = 0;

      Ok(SuccessAndFailureStats {
         total_success_count: lock.total_success_count,
         total_failure_count: lock.total_failure_count,
         consecutive_success_count: lock.consecutive_success_count,
         consecutive_failure_count: lock.consecutive_failure_count,
         maybe_current_job: lock.maybe_current_job.as_ref().map(|job| {
            JobDetail {
               job_token: job.job_token.clone(),
               job_started_at: job.job_started_at,
            }
         }),
      })
   }

   pub fn record_job_start(&self, job_token: &str) -> AnyhowResult<SuccessAndFailureStats> {
      // NB: lock errors can't be moved between threads, so we change their type
      let mut lock = self.inner.write()
          .map_err(|e| anyhow!("lock error: {:?}", e))?;

      Ok(SuccessAndFailureStats {
         total_success_count: lock.total_success_count,
         total_failure_count: lock.total_failure_count,
         consecutive_success_count: lock.consecutive_success_count,
         consecutive_failure_count: lock.consecutive_failure_count,
         maybe_current_job: Some(JobDetail {
            job_token: job_token.to_string(),
            job_started_at: Utc::now(),
         }),
      })
   }

   pub fn record_job_end(&self) -> AnyhowResult<SuccessAndFailureStats> {
      // NB: lock errors can't be moved between threads, so we change their type
      let mut lock = self.inner.write()
          .map_err(|e| anyhow!("lock error: {:?}", e))?;

      Ok(SuccessAndFailureStats {
         total_success_count: lock.total_success_count,
         total_failure_count: lock.total_failure_count,
         consecutive_success_count: lock.consecutive_success_count,
         consecutive_failure_count: lock.consecutive_failure_count,
         maybe_current_job: None,
      })
   }
}
