-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Add 'vits' to the tts_model_type.

ALTER TABLE tts_models
MODIFY COLUMN tts_model_type ENUM(
    'not-set',
    'tacotron2',
    'glowtts',
    'glowtts-vocodes',
    'talknet',
    'vits'
) NOT NULL DEFAULT 'not-set';

-- Technically we won't be using the model upload jobs table anymore,
-- but add 'vits' for consistency.

ALTER TABLE tts_model_upload_jobs
MODIFY COLUMN tts_model_type ENUM(
    'not-set',
    'tacotron2',
    'glowtts',
    'glowtts-vocodes',
    'talknet',
    'vits'
) NOT NULL DEFAULT 'not-set';
