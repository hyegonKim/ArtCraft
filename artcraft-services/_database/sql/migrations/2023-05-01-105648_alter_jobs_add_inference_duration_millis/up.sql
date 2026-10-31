-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_download_jobs
    ADD COLUMN success_inference_execution_millis INT(10) UNSIGNED DEFAULT NULL
    AFTER success_execution_millis;

ALTER TABLE generic_inference_jobs
    ADD COLUMN success_inference_execution_millis INT(10) UNSIGNED DEFAULT NULL
    AFTER success_execution_millis;
