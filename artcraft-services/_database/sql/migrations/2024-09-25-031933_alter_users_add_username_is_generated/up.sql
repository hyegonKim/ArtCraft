-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE users
    ADD COLUMN
        username_is_generated BOOLEAN NOT NULL DEFAULT false
    AFTER display_name;
