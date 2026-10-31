-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_uploads DROP COLUMN public_bucket_directory_full_path;

ALTER TABLE media_uploads
    RENAME COLUMN extra_file_modification_info TO maybe_extra_file_modification_info;
