-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- TODO/FIXME: I originally created this with the wrong directory month.
--  I'll have to repair the migrations table.
--  version: 20210925010101 | run_on: 2021-08-25 03:01:15 (development)
--  UPDATE __diesel_schema_migrations SET version=20210825010101 WHERE version=20210925010101 LIMIT 1;
-- NB: Fixed in production on 2022-01-06

ALTER TABLE tts_models
ADD COLUMN maybe_default_pretrained_vocoder VARCHAR(64) DEFAULT NULL
AFTER text_preprocessing_algorithm;

ALTER TABLE tts_inference_jobs
ADD COLUMN maybe_override_pretrained_vocoder VARCHAR(64) DEFAULT NULL
AFTER model_token;

ALTER TABLE tts_results
ADD COLUMN maybe_pretrained_vocoder_used VARCHAR(64) DEFAULT NULL
AFTER model_token;
