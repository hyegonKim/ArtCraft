-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN maybe_model_type VARCHAR(32) DEFAULT NULL
    AFTER inference_category;

CREATE INDEX index_maybe_model_type_and_maybe_model_token ON generic_inference_jobs (maybe_model_type, maybe_model_token);
CREATE INDEX index_maybe_model_type ON generic_inference_jobs (maybe_model_type);
