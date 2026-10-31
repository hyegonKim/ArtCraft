-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
ADD COLUMN maybe_external_third_party VARCHAR(16) DEFAULT NULL;

ALTER TABLE generic_inference_jobs
ADD COLUMN maybe_external_third_party_id VARCHAR(64) DEFAULT NULL;
