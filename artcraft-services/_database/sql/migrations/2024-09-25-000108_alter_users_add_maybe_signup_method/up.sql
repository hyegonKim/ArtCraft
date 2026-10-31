-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN
        maybe_signup_method VARCHAR(32) DEFAULT NULL
    AFTER maybe_source;
