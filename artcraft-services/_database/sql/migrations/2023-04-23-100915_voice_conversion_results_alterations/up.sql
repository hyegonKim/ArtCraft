-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- There isn't a vocoder, and even if so, it doesn't need to be referenced here.

DROP INDEX `fk_maybe_vocoder_token` ON `voice_conversion_results`;

ALTER TABLE voice_conversion_results DROP COLUMN maybe_vocoder_token;

ALTER TABLE voice_conversion_results
    ADD COLUMN media_token_type VARCHAR(32) NOT NULL
    AFTER media_token;

ALTER TABLE voice_conversion_results
    ADD COLUMN generated_by_cluster VARCHAR(255) NOT NULL
    AFTER generated_by_worker;

CREATE INDEX `fk_media_token_and_media_token_type`
    ON `voice_conversion_results` (`media_token`, `media_token_type`);
