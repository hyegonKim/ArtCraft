-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN frontend_failure_category VARCHAR(32) DEFAULT NULL
    AFTER internal_debugging_failure_reason;
