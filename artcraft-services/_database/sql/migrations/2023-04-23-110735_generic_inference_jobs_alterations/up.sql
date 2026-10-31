-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE generic_inference_jobs
    ADD COLUMN maybe_input_source_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_model_token;

ALTER TABLE generic_inference_jobs
    ADD COLUMN maybe_input_source_token_type VARCHAR(32) DEFAULT NULL
    AFTER maybe_input_source_token;

CREATE INDEX `fk_maybe_input_source_token`
    ON `generic_inference_jobs` (`maybe_input_source_token`);

CREATE INDEX `fk_maybe_input_source_token_and_type`
    ON `generic_inference_jobs` (`maybe_input_source_token`, `maybe_input_source_token_type`);
