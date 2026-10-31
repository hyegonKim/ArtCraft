-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
    ADD COLUMN text_pipeline_type VARCHAR(64) DEFAULT NULL
    AFTER tts_model_type;
