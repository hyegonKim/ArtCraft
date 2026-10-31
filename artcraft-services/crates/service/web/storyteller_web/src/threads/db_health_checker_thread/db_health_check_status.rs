use std::sync::{Arc, RwLock};

use anyhow::anyhow;
use chrono::NaiveDateTime;

use crate::AnyhowResult;

#[derive(Clone)]
pub struct HealthCheckStatus {
  internal_status: Arc<RwLock<HealthCheckStatusData>>,
}

#[derive(Clone)]
pub struct HealthCheckStatusData {
  pub last_db_time: Option<NaiveDateTime>,
  pub is_healthy: bool,
  /// Number of consecutive tries the DB has been good.
  /// Resets at the first sign of bad.
  pub healthy_check_consecutive_count: Option<u64>,
  /// Number of consecutive tries the DB has been bad.
  /// Resets at the first sign of good.
  pub unhealthy_check_consecutive_count: Option<u64>,
}

impl HealthCheckStatus {

  pub fn new() -> Self {
    Self {
      internal_status: Arc::new(RwLock::new(HealthCheckStatusData {
        last_db_time: None,
        is_healthy: false, // Start false
        healthy_check_consecutive_count: None,
        unhealthy_check_consecutive_count: None,
      })),
    }
  }

  pub fn record_ping_success(&self, db_time: NaiveDateTime) -> AnyhowResult<()> {
    match self.internal_status.write() {
      Err(_) => Err(anyhow!("Can't write lock")),
      Ok(mut lock) => {
        (*lock).last_db_time = Some(db_time);
        (*lock).is_healthy = true;
        (*lock).unhealthy_check_consecutive_count = None;

        let healthy_count = (*lock).healthy_check_consecutive_count.unwrap_or(0);
        (*lock).healthy_check_consecutive_count = Some(healthy_count.saturating_add(1));

        Ok(())
      },
    }
  }

  pub fn record_ping_failure(&self) -> AnyhowResult<()> {
    match self.internal_status.write() {
      Err(_) => Err(anyhow!("Can't write lock")),
      Ok(mut lock) => {
        (*lock).is_healthy = false;
        (*lock).healthy_check_consecutive_count = None;

        let unhealthy_count = (*lock).unhealthy_check_consecutive_count.unwrap_or(0);
        (*lock).unhealthy_check_consecutive_count = Some(unhealthy_count.saturating_add(1));

        Ok(())
      },
    }
  }

  /// Read the status
  pub fn get_health_check_status(&self) -> AnyhowResult<HealthCheckStatusData> {
    match self.internal_status.read() {
      Err(_) => Err(anyhow!("Can't read lock")),
      Ok(lock) => {
        Ok(lock.clone())
      },
    }
  }
}
