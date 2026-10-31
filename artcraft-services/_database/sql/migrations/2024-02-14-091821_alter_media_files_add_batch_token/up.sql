-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_batch_token VARCHAR(32) DEFAULT NULL
    AFTER is_batch_generated;

ALTER TABLE media_files
    ADD INDEX index_maybe_batch_token (maybe_batch_token);
