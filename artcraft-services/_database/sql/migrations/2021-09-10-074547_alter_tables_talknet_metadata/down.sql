-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
DROP COLUMN private_bucket_object_is_archive;

ALTER TABLE tts_models
DROP COLUMN has_self_contained_vocoder;

ALTER TABLE tts_models
DROP COLUMN has_self_contained_duration_model;

ALTER TABLE tts_models
DROP COLUMN has_self_contained_pitch_model;
