-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Revert the rename
ALTER TABLE media_files RENAME COLUMN maybe_creator_file_synthetic_id TO maybe_creator_synthetic_id ;

ALTER TABLE media_files
DROP COLUMN maybe_creator_category_synthetic_id;
