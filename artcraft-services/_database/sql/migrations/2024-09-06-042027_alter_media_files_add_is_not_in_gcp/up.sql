-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN
        is_not_in_gcp BOOLEAN NOT NULL DEFAULT false
    AFTER is_in_aws;

