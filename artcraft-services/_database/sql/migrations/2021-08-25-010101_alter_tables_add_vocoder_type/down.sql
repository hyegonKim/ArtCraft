-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
    DROP COLUMN maybe_default_pretrained_vocoder;

ALTER TABLE tts_inference_jobs
    DROP COLUMN maybe_override_pretrained_vocoder;

ALTER TABLE tts_results
    DROP COLUMN maybe_pretrained_vocoder_used;
