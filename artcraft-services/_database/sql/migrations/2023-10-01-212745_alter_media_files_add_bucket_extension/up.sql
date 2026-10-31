-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_public_bucket_extension VARCHAR(16) DEFAULT NULL
    AFTER public_bucket_directory_hash;
