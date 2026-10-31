-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_uploads
    ADD COLUMN public_bucket_directory_full_path VARCHAR(255) NOT NULL
    AFTER public_bucket_directory_hash;

ALTER TABLE media_uploads
    RENAME COLUMN maybe_extra_file_modification_info TO extra_file_modification_info ;
