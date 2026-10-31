-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- A bot command prefix, eg. 'sonic' that could be interpreted as '/sonic' by a TTS system.
-- These are unique strings and can only be applied to one model at a time.
-- Only moderators can set this, and it's only for the best voices.
ALTER TABLE tts_models
    ADD COLUMN maybe_suggested_unique_bot_command VARCHAR(255) DEFAULT NULL
    AFTER is_twitch_featured;

-- And the unique index.
ALTER TABLE tts_models
    ADD UNIQUE INDEX unique_index_maybe_suggested_unique_bot_command (maybe_suggested_unique_bot_command);
