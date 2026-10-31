-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_download_jobs DROP COLUMN success_inference_execution_millis;
ALTER TABLE generic_inference_jobs DROP COLUMN success_inference_execution_millis;
