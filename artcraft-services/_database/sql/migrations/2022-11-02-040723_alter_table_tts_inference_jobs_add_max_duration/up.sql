-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs
    ADD COLUMN max_duration_seconds INTEGER NOT NULL DEFAULT 0
    AFTER is_debug_request;
