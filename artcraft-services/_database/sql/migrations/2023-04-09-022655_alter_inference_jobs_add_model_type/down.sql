-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX index_maybe_model_type_and_maybe_model_token ON generic_inference_jobs;
DROP INDEX index_maybe_model_type ON generic_inference_jobs;

ALTER TABLE generic_inference_jobs DROP COLUMN maybe_model_type;
