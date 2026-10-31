-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE model_weights_extension_tts_details
    ADD COLUMN text_pipeline_type VARCHAR(64) DEFAULT NULL
    AFTER ietf_primary_language_subtag;
