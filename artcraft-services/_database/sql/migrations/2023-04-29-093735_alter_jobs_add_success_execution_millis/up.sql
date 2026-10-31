-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_download_jobs
    ADD COLUMN success_execution_millis INT(10) UNSIGNED DEFAULT NULL
    AFTER failure_reason;

ALTER TABLE generic_inference_jobs
    ADD COLUMN success_execution_millis INT(10) UNSIGNED DEFAULT NULL
    AFTER internal_debugging_failure_reason;
