//! Follow a freshly enqueued job through the status endpoints, printing every
//! step: the cheap batch status while it runs, then the full record once it's
//! done. Shared by the enqueue live tests.

use crate::endpoints::jobs::job_status::{job_status, JobStatusArgs, JobStatusRequest, JobStatusResponse};
use crate::endpoints::jobs::job_status_batch::{job_status_batch, JobStatusBatchArgs, JobStatusBatchRequest};
use crate::session::higgsfield_session::HiggsfieldSession;
use crate::types::ids::JobId;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

pub async fn poll_job_to_completion(session: &HiggsfieldSession, job_id: &JobId) -> anyhow::Result<JobStatusResponse> {
  let started = Instant::now();
  let ids = vec![job_id.clone()];

  loop {
    // Tokens live ~60s; ask the session each time so long jobs don't 401.
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let batch = job_status_batch(JobStatusBatchArgs {
      request: JobStatusBatchRequest { ids: ids.clone() },
      auth: &auth,
      host: session.api_host(),
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let item = batch.find(job_id)
        .ok_or_else(|| anyhow::anyhow!("job {job_id} absent from batch response: {:?}", batch))?;
    println!(
      "[{:>4}s] POST /fnf/jobs/status-batch => {} status={} job_set_type={:?}",
      started.elapsed().as_secs(), item.id, item.status, item.job_set_type,
    );

    if item.status.is_terminal() {
      let job = job_status(JobStatusArgs {
        request: JobStatusRequest { job_id: job_id.clone() },
        auth: &auth,
        host: session.api_host(),
      }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

      println!("\n===== GET /fnf/jobs/{job_id} (final) =====\n{:#?}", job);
      println!("result_url = {:?}", job.result_url());
      anyhow::ensure!(job.status.is_success(), "job {job_id} ended in state {} (reason: {:?})", job.status, job.fail_reason());
      return Ok(job);
    }

    anyhow::ensure!(started.elapsed() < TIMEOUT, "job {job_id} did not finish within {:?}", TIMEOUT);
    tokio::time::sleep(POLL_INTERVAL).await;
  }
}

/// Like [`poll_job_to_completion`] for a whole batch: polls `status-batch`
/// until every job is terminal, then fetches and prints each full record.
/// Fails if any job ended unsuccessfully.
pub async fn poll_jobs_to_completion(session: &HiggsfieldSession, job_ids: &[JobId]) -> anyhow::Result<Vec<JobStatusResponse>> {
  let started = Instant::now();

  loop {
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let batch = job_status_batch(JobStatusBatchArgs {
      request: JobStatusBatchRequest { ids: job_ids.to_vec() },
      auth: &auth,
      host: session.api_host(),
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let mut all_terminal = true;
    for job_id in job_ids {
      let item = batch.find(job_id)
          .ok_or_else(|| anyhow::anyhow!("job {job_id} absent from batch response: {:?}", batch))?;
      println!("[{:>4}s] status-batch => {} status={}", started.elapsed().as_secs(), item.id, item.status);
      all_terminal &= item.status.is_terminal();
    }

    if all_terminal {
      let mut jobs = Vec::with_capacity(job_ids.len());
      for job_id in job_ids {
        let job = job_status(JobStatusArgs {
          request: JobStatusRequest { job_id: job_id.clone() },
          auth: &auth,
          host: session.api_host(),
        }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
        println!(
          "\n===== GET /fnf/jobs/{job_id} (final) =====\nstatus={} type={} size={:?}x{:?} quality={:?} thinking={:?} seed={:?}\nresult_url = {:?}",
          job.status, job.job_set_type, job.params.width, job.params.height, job.params.quality, job.params.thinking, job.params.seed, job.result_url(),
        );
        anyhow::ensure!(job.status.is_success(), "job {job_id} ended in state {} (reason: {:?})", job.status, job.fail_reason());
        jobs.push(job);
      }
      return Ok(jobs);
    }

    anyhow::ensure!(started.elapsed() < TIMEOUT, "jobs did not finish within {:?}", TIMEOUT);
    tokio::time::sleep(POLL_INTERVAL).await;
  }
}
