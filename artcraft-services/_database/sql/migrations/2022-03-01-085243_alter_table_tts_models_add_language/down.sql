-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
    DROP COLUMN ietf_language_tag;

ALTER TABLE tts_models
    DROP COLUMN ietf_primary_language_subtag;
