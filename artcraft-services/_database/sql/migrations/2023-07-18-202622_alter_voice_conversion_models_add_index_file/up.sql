-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE voice_conversion_models
    ADD COLUMN has_index_file BOOLEAN NOT NULL DEFAULT FALSE
    AFTER ietf_primary_language_subtag;
