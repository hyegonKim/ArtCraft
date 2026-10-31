-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

DROP INDEX `fk_maybe_custom_vocoder_token` ON `tts_models`;

ALTER TABLE tts_models DROP COLUMN maybe_custom_vocoder_token;
