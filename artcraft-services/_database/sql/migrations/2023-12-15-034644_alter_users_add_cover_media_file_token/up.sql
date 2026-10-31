-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN maybe_cover_media_file_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_avatar_media_file_token;

ALTER TABLE users
    ADD INDEX index_maybe_cover_media_file_token (maybe_cover_media_file_token);
