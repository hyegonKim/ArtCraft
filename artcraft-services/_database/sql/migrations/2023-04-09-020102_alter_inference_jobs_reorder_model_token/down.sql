-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs MODIFY maybe_model_token VARCHAR(32) DEFAULT NULL AFTER maybe_raw_inference_text;
