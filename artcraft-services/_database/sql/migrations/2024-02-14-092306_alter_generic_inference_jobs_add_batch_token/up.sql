-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN on_success_result_batch_token VARCHAR(32) DEFAULT NULL
    AFTER on_success_result_entity_token;
