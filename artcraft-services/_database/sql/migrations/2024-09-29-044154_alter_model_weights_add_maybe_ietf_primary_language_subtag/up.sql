-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights
    ADD COLUMN
      maybe_ietf_primary_language_subtag VARCHAR(12) DEFAULT NULL
    AFTER maybe_ietf_language_tag;

