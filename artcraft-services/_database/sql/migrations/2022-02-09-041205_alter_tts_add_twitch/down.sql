-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs DROP COLUMN is_for_twitch;
ALTER TABLE tts_inference_jobs DROP COLUMN is_from_api;
ALTER TABLE tts_inference_jobs DROP COLUMN priority_level;

ALTER TABLE tts_results DROP COLUMN is_from_api;
ALTER TABLE tts_results DROP COLUMN is_for_twitch;
