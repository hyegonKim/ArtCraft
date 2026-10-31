-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    MODIFY COLUMN password_version INT UNSIGNED NOT NULL DEFAULT 0;

ALTER TABLE users
    MODIFY COLUMN version INT UNSIGNED NOT NULL DEFAULT 0;

