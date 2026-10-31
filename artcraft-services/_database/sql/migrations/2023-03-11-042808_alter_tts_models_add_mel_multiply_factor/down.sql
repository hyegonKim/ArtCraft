-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models DROP COLUMN maybe_custom_mel_multiply_factor;
ALTER TABLE tts_models DROP COLUMN use_default_mel_multiply_factor;
