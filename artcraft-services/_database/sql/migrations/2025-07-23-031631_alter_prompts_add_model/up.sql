-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE prompts
    ADD COLUMN maybe_model_type VARCHAR(32) DEFAULT NULL;
