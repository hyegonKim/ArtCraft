-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Optional internal-only debugging information in the case of failure.
ALTER TABLE tts_inference_jobs
    ADD COLUMN internal_debugging_failure_reason VARCHAR(512) DEFAULT NULL
    AFTER failure_reason;

-- The last worker (hostname or pod name) to touch the job, either in the case of success or failure.
ALTER TABLE tts_inference_jobs
    ADD COLUMN last_assigned_worker VARCHAR(512) DEFAULT NULL
    AFTER internal_debugging_failure_reason;
