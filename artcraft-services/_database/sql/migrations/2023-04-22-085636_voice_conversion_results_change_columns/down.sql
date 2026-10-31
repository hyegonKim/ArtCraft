-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE voice_conversion_results DROP COLUMN public_bucket_hash;
ALTER TABLE voice_conversion_results DROP COLUMN bucket_has_wav;
ALTER TABLE voice_conversion_results DROP COLUMN bucket_has_mp3;
ALTER TABLE voice_conversion_results DROP COLUMN bucket_has_mp4;
ALTER TABLE voice_conversion_results DROP COLUMN bucket_has_webm;
ALTER TABLE voice_conversion_results DROP COLUMN bucket_has_spectrogram;

ALTER TABLE voice_conversion_results
    ADD COLUMN public_bucket_directory_full_path VARCHAR(255) NOT NULL
    AFTER maybe_creator_synthetic_id;

ALTER TABLE voice_conversion_results
    ADD COLUMN public_bucket_wav_audio_object_name VARCHAR(255) NOT NULL
    AFTER public_bucket_directory_full_path;

ALTER TABLE voice_conversion_results
    ADD COLUMN public_bucket_spectrogram_object_name VARCHAR(255) NOT NULL
    AFTER public_bucket_wav_audio_object_name;

ALTER TABLE voice_conversion_results
    ADD COLUMN public_bucket_video_object_name VARCHAR(255) NOT NULL
    AFTER public_bucket_spectrogram_object_name;
