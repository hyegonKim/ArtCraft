-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN can_access_studio BOOLEAN NOT NULL DEFAULT false
    AFTER user_role_slug;

ALTER TABLE users
    ADD INDEX index_can_access_studio (can_access_studio);
