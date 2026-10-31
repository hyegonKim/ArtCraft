-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Kill the column
ALTER TABLE media_files
DROP COLUMN public_bucket_directory_full_path;
