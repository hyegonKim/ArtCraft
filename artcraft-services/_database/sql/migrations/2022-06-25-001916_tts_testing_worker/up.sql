-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs
    ADD COLUMN is_debug_request BOOLEAN NOT NULL DEFAULT FALSE
    AFTER is_for_twitch;

ALTER TABLE tts_inference_jobs
    ADD INDEX index_is_debug_request (is_debug_request);

ALTER TABLE tts_results
    ADD COLUMN is_debug_request BOOLEAN NOT NULL DEFAULT FALSE
    AFTER is_for_twitch;
