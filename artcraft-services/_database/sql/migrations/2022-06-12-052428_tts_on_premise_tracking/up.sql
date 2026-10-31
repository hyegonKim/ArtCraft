-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_results
    ADD COLUMN is_generated_on_prem BOOLEAN NOT NULL DEFAULT FALSE
    AFTER is_for_twitch;

ALTER TABLE tts_results
    ADD COLUMN generated_by_worker VARCHAR(255) DEFAULT NULL
    AFTER is_generated_on_prem;
