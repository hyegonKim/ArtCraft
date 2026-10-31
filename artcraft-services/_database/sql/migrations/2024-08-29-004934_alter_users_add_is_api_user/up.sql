-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN is_api_user BOOLEAN NOT NULL DEFAULT false
    AFTER can_access_studio;

ALTER TABLE users
    ADD INDEX index_is_api_user (is_api_user);
