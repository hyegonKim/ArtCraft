-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_download_jobs
    ADD COLUMN assigned_worker VARCHAR(128) DEFAULT NULL
    AFTER failure_reason;

ALTER TABLE generic_download_jobs
    ADD COLUMN assigned_cluster VARCHAR(128) DEFAULT NULL
    AFTER assigned_worker;

ALTER TABLE generic_download_jobs
    ADD COLUMN first_started_at TIMESTAMP NULL
    AFTER retry_at;

ALTER TABLE generic_download_jobs
    ADD COLUMN successfully_completed_at TIMESTAMP NULL
    AFTER first_started_at;
