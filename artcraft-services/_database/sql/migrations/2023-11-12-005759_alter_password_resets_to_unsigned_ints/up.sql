-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE user_password_resets
    MODIFY COLUMN current_password_version INT UNSIGNED NOT NULL DEFAULT 0;

ALTER TABLE user_password_resets
    MODIFY COLUMN version INT UNSIGNED NOT NULL DEFAULT 0;
