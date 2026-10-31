-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- Delete TTS results for a single model
-- Do this in short batches so a lock isn't held for prohibitively long.
update tts_results
set mod_deleted_at = NOW()
where model_token = 'TM:ztt5s1be5tq6'
  and mod_deleted_at IS NULL
    limit 5000;
