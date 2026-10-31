-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

ALTER TABLE tts_inference_jobs
  ADD COLUMN is_from_api BOOLEAN NOT NULL DEFAULT FALSE
  AFTER creator_set_visibility,

  ADD COLUMN is_for_twitch BOOLEAN NOT NULL DEFAULT FALSE
  AFTER is_from_api,

  -- Priority *increases*, so a level of 2 will be higher than 1.
  -- By default, all jobs from FakeYou have level 0.
  -- Twitch TTS has level 1.
  -- Paid Twitch and Paid API have level 2.
  ADD COLUMN priority_level TINYINT UNSIGNED NOT NULL DEFAULT 0
  AFTER status;

ALTER TABLE tts_results
  ADD COLUMN is_from_api BOOLEAN NOT NULL DEFAULT FALSE
  AFTER duration_millis,
  ADD COLUMN is_for_twitch BOOLEAN NOT NULL DEFAULT FALSE
  AFTER is_from_api;
