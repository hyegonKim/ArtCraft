-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE prompts
    ADD COLUMN maybe_generation_provider VARCHAR(16) DEFAULT NULL;
