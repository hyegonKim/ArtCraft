-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE twitch_channel_point_events
    DROP COLUMN created_at;

ALTER TABLE twitch_channel_point_events
    DROP COLUMN updated_at;
