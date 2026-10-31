-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX `fk_media_token_and_media_token_type` ON `voice_conversion_results`;

ALTER TABLE voice_conversion_results DROP COLUMN media_token_type;
ALTER TABLE voice_conversion_results DROP COLUMN generated_by_cluster;

ALTER TABLE voice_conversion_results
    ADD COLUMN maybe_vocoder_token VARCHAR(64) DEFAULT NULL
    AFTER model_token;

CREATE INDEX `fk_maybe_vocoder_token`
    ON `voice_conversion_results` (`maybe_vocoder_token`);
