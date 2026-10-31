-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models DROP COLUMN user_ratings_total_count;
ALTER TABLE tts_models DROP COLUMN user_ratings_positive_count;
ALTER TABLE tts_models DROP COLUMN user_ratings_negative_count;
