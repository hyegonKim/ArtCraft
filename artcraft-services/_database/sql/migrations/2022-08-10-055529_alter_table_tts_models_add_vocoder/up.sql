-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- If set, we'll use a custom vocoder (from table vocoder_models) instead of a default vocoder.
-- If not set, we'll use `maybe_default_pretrained_vocoder` (or the website default).
ALTER TABLE tts_models
ADD COLUMN maybe_custom_vocoder_token VARCHAR(32) DEFAULT NULL
    AFTER maybe_default_pretrained_vocoder;

CREATE INDEX `fk_maybe_custom_vocoder_token` ON `tts_models` (`maybe_custom_vocoder_token`);
