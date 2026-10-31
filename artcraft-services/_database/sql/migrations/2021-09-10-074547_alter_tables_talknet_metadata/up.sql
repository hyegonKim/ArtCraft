-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_models
ADD COLUMN private_bucket_object_is_archive BOOLEAN NOT NULL DEFAULT FALSE
AFTER private_bucket_object_name;

ALTER TABLE tts_models
ADD COLUMN has_self_contained_vocoder BOOLEAN NOT NULL DEFAULT FALSE
AFTER tts_model_type;

ALTER TABLE tts_models
ADD COLUMN has_self_contained_duration_model BOOLEAN NOT NULL DEFAULT FALSE
AFTER has_self_contained_vocoder;

ALTER TABLE tts_models
ADD COLUMN has_self_contained_pitch_model BOOLEAN NOT NULL DEFAULT FALSE
AFTER has_self_contained_duration_model;
