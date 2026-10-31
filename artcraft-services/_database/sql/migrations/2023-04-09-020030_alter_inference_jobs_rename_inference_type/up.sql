-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs RENAME COLUMN inference_type TO inference_category;

ALTER TABLE generic_inference_jobs RENAME INDEX index_inference_type TO index_inference_category;
