//! This crate is simply meant to collect common shared legacy "jobs" components
//! It's an artifact of refactoring and maybe should mostly disappear.
pub mod job_progress_reporter;
pub mod job_stats; // NB: This seems valuable for all jobs
pub mod noop_logger;
pub mod redis_job_status_logger;
pub mod semi_persistent_cache_dir;