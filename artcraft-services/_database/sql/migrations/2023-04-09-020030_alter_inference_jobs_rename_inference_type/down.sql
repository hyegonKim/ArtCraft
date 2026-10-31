-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs RENAME COLUMN inference_category TO inference_type;

ALTER TABLE generic_inference_jobs RENAME INDEX index_inference_category TO index_inference_type;
