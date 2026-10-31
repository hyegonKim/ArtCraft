-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN
        username_is_not_customized BOOLEAN NOT NULL DEFAULT false
    AFTER username_is_generated;
