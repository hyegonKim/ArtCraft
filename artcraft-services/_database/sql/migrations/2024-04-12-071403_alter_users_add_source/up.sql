-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN
        maybe_source VARCHAR(255) DEFAULT NULL
    AFTER auto_play_video_preference;
