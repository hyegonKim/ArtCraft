use chrono::{DateTime, Duration, TimeZone, Utc};
use log::info;

/// Log after periods of no work, but prevent spamming stdout.
pub struct NoOpLogger {
  last_log_time: DateTime<Utc>,
  logless_duration: Duration,
  op_count: u64,
}

impl NoOpLogger {
  pub fn new(log_every_millis: i64) -> Self {
    Self {
      last_log_time: Utc.timestamp(0, 0),
      logless_duration: Duration::milliseconds(log_every_millis),
      op_count: 0
    }
  }

  pub fn log_after_awhile(&mut self) {
    let now = Utc::now();
    let delta = now - self.last_log_time;

    if delta > self.logless_duration {
      info!("No operations for {:?}. {} operations.", delta, self.op_count);
      self.op_count = 0;
      self.last_log_time  = now;
    }

    self.op_count += 1;
  }

  pub fn log_message_after_awhile(&mut self, message: &str) {
    let now = Utc::now();
    let delta = now - self.last_log_time;

    if delta > self.logless_duration {
      info!("No operations for {:?}. {} operations: {}", delta, self.op_count, message);
      self.op_count = 0;
      self.last_log_time  = now;
    }

    self.op_count += 1;
  }
}