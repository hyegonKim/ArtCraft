-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_engine_category VARCHAR(16) DEFAULT NULL
    AFTER maybe_frame_height;

CREATE INDEX index_maybe_engine_category ON media_files (maybe_engine_category);
