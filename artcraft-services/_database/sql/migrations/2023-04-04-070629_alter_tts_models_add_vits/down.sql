-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- REVERT ADDING 'vits' MODEL TYPE

ALTER TABLE tts_models
MODIFY COLUMN tts_model_type ENUM(
    'not-set',
    'tacotron2',
    'glowtts',
    'glowtts-vocodes',
    'talknet'
) NOT NULL DEFAULT 'not-set';

ALTER TABLE tts_model_upload_jobs
MODIFY COLUMN tts_model_type ENUM(
    'not-set',
    'tacotron2',
    'glowtts',
    'glowtts-vocodes',
    'talknet'
) NOT NULL DEFAULT 'not-set';
