-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE media_files
    ADD COLUMN maybe_text_transcript TEXT DEFAULT NULL
    AFTER maybe_frame_height;
