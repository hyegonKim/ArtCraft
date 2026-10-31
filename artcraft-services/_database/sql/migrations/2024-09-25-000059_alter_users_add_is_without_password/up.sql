-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN
        is_without_password BOOLEAN NOT NULL DEFAULT false
    AFTER password_version;
