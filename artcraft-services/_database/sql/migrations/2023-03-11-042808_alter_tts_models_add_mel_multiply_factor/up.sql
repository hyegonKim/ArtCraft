-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- If true, multiply the mel outputs before being vocoded by a globally default constant.
ALTER TABLE tts_models
    ADD COLUMN use_default_mel_multiply_factor BOOLEAN NOT NULL DEFAULT FALSE
    AFTER text_preprocessing_algorithm;

-- If not null, multiply the mel outputs before being vocoded by this value.
-- This is used instead of the default if `use_default_mel_multiply_factor` is
-- set (ie. `use_default_mel_multiply_factor` is ignored and this custom value is used instead)
ALTER TABLE tts_models
    ADD COLUMN maybe_custom_mel_multiply_factor DOUBLE DEFAULT NULL
    AFTER use_default_mel_multiply_factor;
