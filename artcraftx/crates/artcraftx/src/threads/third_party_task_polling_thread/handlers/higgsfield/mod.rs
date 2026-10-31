//! First-party (cookie-session) Higgsfield jobs. A task's `provider_job_id`
//! holds every job id of the Higgsfield job set (comma-separated for a
//! batch); the poller checks them in one batch-status call per account and,
//! once every job is terminal, downloads the finished files and hands them
//! to the shared completion routine.

pub mod handle_higgsfield_complete;
pub mod handle_higgsfield_failure;
pub mod higgsfield_failure_reason;
pub mod higgsfield_poll_sessions;
pub mod poll_higgsfield_tasks;
