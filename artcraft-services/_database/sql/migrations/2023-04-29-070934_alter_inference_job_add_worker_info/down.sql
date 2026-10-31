-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs DROP COLUMN first_started_at;
ALTER TABLE generic_inference_jobs DROP COLUMN assigned_cluster;
ALTER TABLE generic_inference_jobs DROP COLUMN assigned_worker;
ALTER TABLE generic_inference_jobs DROP COLUMN successfully_completed_at;
