-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- We'll have one ordering by files
ALTER TABLE media_files RENAME COLUMN maybe_creator_synthetic_id TO maybe_creator_file_synthetic_id;

-- And another ordering by category
ALTER TABLE media_files
    ADD COLUMN maybe_creator_category_synthetic_id BIGINT(20) DEFAULT NULL
    AFTER maybe_creator_file_synthetic_id;
