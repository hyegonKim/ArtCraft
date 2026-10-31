-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX `fk_maybe_input_source_token` ON `generic_inference_jobs`;
DROP INDEX `fk_maybe_input_source_token_and_type` ON `generic_inference_jobs`;

ALTER TABLE generic_inference_jobs DROP COLUMN maybe_input_source_token;
ALTER TABLE generic_inference_jobs DROP COLUMN maybe_input_source_token_type;
