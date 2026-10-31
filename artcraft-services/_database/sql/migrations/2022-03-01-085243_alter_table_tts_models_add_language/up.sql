-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
ALTER TABLE tts_models
    ADD COLUMN ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en'
    AFTER maybe_default_pretrained_vocoder;

-- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es")
ALTER TABLE tts_models
    ADD COLUMN ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en'
    AFTER ietf_language_tag;
