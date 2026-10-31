//! datetimes
//!
//! While "chrono" is the preferred datetime crate, we still leverage some
//! stuff in "time" and std. This crate contains some additional constants
//! and utility methods around datetimes and durations not present in the
//! primary 3rd party and standard libraries.
//!

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::Lazy;

pub mod generate_dates_inclusive;

// NB: Chrono doesn't have any 'const fn's of not to leverage.
/// This is the unix epoch datetime.
pub static CHRONO_DATETIME_UNIX_EPOCH : Lazy<DateTime<Utc>> = Lazy::new(|| {
  DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc)
});

#[cfg(test)]
mod tests {
  use crate::CHRONO_DATETIME_UNIX_EPOCH;

  #[test]
  fn test_chrono_datetime_unix_epoch() {
    assert_eq!(CHRONO_DATETIME_UNIX_EPOCH.timestamp(), 0)
  }
}
