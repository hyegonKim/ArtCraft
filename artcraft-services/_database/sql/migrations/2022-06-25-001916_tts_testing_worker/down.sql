-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs
    DROP COLUMN is_debug_request;

ALTER TABLE tts_results
    DROP COLUMN is_debug_request;

