-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
    ADD COLUMN is_front_page_featured BOOLEAN NOT NULL DEFAULT FALSE
    AFTER version_string,

    ADD COLUMN is_twitch_featured BOOLEAN NOT NULL DEFAULT FALSE
    AFTER is_front_page_featured;
