-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE twitch_bits_events
    DROP COLUMN created_at;

ALTER TABLE twitch_bits_events
    DROP COLUMN updated_at;
