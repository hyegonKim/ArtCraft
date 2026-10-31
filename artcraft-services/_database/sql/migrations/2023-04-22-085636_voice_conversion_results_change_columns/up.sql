-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- These columns are no longer useful.
-- It's better to store booleans for each type of file we store and calculate the paths.

ALTER TABLE voice_conversion_results DROP COLUMN public_bucket_directory_full_path;
ALTER TABLE voice_conversion_results DROP COLUMN public_bucket_wav_audio_object_name;
ALTER TABLE voice_conversion_results DROP COLUMN public_bucket_spectrogram_object_name;
ALTER TABLE voice_conversion_results DROP COLUMN public_bucket_video_object_name;

-- The hash will contain the key information for where files live

ALTER TABLE voice_conversion_results
    ADD COLUMN public_bucket_hash VARCHAR(64) NOT NULL
    AFTER maybe_creator_synthetic_id;

-- The boolean flags will tell what we store.

ALTER TABLE voice_conversion_results
    ADD COLUMN bucket_has_wav BOOLEAN NOT NULL DEFAULT FALSE
    AFTER public_bucket_hash;

ALTER TABLE voice_conversion_results
    ADD COLUMN bucket_has_mp3 BOOLEAN NOT NULL DEFAULT FALSE
    AFTER bucket_has_wav;

ALTER TABLE voice_conversion_results
    ADD COLUMN bucket_has_mp4 BOOLEAN NOT NULL DEFAULT FALSE
    AFTER bucket_has_mp3;

ALTER TABLE voice_conversion_results
    ADD COLUMN bucket_has_webm BOOLEAN NOT NULL DEFAULT FALSE
    AFTER bucket_has_mp4;

ALTER TABLE voice_conversion_results
    ADD COLUMN bucket_has_spectrogram BOOLEAN NOT NULL DEFAULT FALSE
    AFTER bucket_has_webm;
