-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN maybe_avatar_media_file_token VARCHAR(32) DEFAULT NULL
    AFTER ip_address_last_update;

ALTER TABLE users
    ADD INDEX index_maybe_avatar_media_file_token (maybe_avatar_media_file_token);
