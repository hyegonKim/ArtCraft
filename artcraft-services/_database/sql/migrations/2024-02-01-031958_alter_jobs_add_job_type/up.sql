-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN job_type VARCHAR(32) DEFAULT NULL
    AFTER uuid_idempotency_token;

ALTER TABLE generic_inference_jobs
    ADD INDEX index_job_type (job_type);
