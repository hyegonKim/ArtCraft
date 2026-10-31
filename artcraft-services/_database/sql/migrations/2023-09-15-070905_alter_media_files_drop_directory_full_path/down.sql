-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN public_bucket_directory_full_path VARCHAR(255) NOT NULL
    AFTER public_bucket_directory_hash;
