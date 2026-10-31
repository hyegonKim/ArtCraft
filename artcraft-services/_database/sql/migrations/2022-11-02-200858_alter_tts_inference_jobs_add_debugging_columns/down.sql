-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs DROP COLUMN last_assigned_worker;

ALTER TABLE tts_inference_jobs DROP COLUMN internal_debugging_failure_reason;
