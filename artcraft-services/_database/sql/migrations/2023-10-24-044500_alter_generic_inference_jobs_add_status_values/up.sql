-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Add two states to allow us to distinguish user-cancelled jobs and system-cancelled jobs.
ALTER TABLE generic_inference_jobs
    MODIFY status ENUM(
        'pending',
        'started',
        'complete_success',
        'complete_failure',
        'attempt_failed',
        'dead',
        'cancelled_by_user',
        'cancelled_by_system'
) NOT NULL DEFAULT 'pending';
