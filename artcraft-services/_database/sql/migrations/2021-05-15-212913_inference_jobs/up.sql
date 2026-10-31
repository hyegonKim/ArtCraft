-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: See the "squashed" copy of this for better documentation
-- and the current state of the table.

CREATE TABLE tts_inference_jobs (
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- This is so the in-progress results can be looked up by the UI.
  token VARCHAR(32) NOT NULL,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== SUCCESS CASE ==========

  -- If the job completes successfully, this is the result token.
  -- This is only populated on a successful result.
  on_success_result_token VARCHAR(32) DEFAULT NULL,

  -- ========== INFERENCE DETAILS ==========

  -- The model to use.
  -- This also determines which architecture we're using.
  model_token VARCHAR(32) NOT NULL,

  -- NB: ADDED BY ALTER
  -- maybe_override_pretrained_vocoder VARCHAR(64) DEFAULT NULL,

  -- The raw, unprocessed user input.
  raw_inference_text TEXT NOT NULL,

  -- ========== CREATOR DETAILS ==========

  -- Foreign key to user
  -- If no user is logged in, this is null.
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- ========== JOB SYSTEM DETAILS ==========

  -- Jobs begin as "pending", then transition to other states.
  --
  --  * Pending = job is ready to go
  --  * Started = job is running
  --  * Complete_Success = job is done (success)
  --  * Complete_Failure = job is done (failure)
  --  * Attempt_Failed = job failed but may retry.
  --  * Dead = job failed permanently.
  --
  -- Pending -> Started -> Complete_Success
  --                    |-> Complete_Failure
  --                    \-> Attempt_Failed -> Started -> { Complete, Failed, Dead }
  status ENUM(
      'pending',
      'started',
      'complete_success',
      'complete_failure',
      'attempt_failed',
      'dead') NOT NULL DEFAULT 'pending',

    -- We can track this against a "max_attempt_count"
  attempt_count INT(3) NOT NULL DEFAULT 0,

  -- If there is a failure, tell the user why.
  failure_reason VARCHAR(512) DEFAULT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- Failed jobs will set a next attempt time.
  -- Subsequent tries can increase the timeout.
  -- Failures because of permissions require human intervention => [retry_at=null].
  -- Failures because of invalid files are dead => [status=dead].
  retry_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),
  KEY fk_on_success_result_token (on_success_result_token),
  KEY fk_model_token (model_token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY index_status (status),
  KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE w2l_inference_jobs (
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- This is so the in-progress results can be looked up by the UI.
  token VARCHAR(32) NOT NULL,

  -- Idempotency token from client
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== SUCCESS CASE ==========

  -- If the job completes successfully, this is the result token.
  -- This is only populated on a successful result.
  on_success_result_token VARCHAR(32) DEFAULT NULL,

  -- ========== INFERENCE DETAILS : FACE TEMPLATE ==========

  -- The W2L template to use
  -- Can be an image or video.
  -- This is null if we're using a custom uploaded image.
  maybe_w2l_template_token VARCHAR(32) DEFAULT NULL,

  -- If we're using a custom uploaded image, this will be present.
  -- NOTE: This may never be supported.
  maybe_public_image_bucket_location VARCHAR(255) DEFAULT NULL,

  -- ========== INFERENCE DETAILS : AUDIO SOURCE ==========

  -- If we're using TTS results, this will be present
  maybe_tts_inference_result_token VARCHAR(32) DEFAULT NULL,

  -- If we're using custom uploaded audio, this will be present.
  maybe_public_audio_bucket_hash VARCHAR(64) DEFAULT NULL,
  maybe_public_audio_bucket_location VARCHAR(255) DEFAULT NULL,

  -- The filename that was used at upload time (if available)
  maybe_original_audio_filename CHAR(255) DEFAULT NULL,

  -- Where the audio file was originally downloaded (if it was downloaded)
  maybe_original_audio_download_url VARCHAR(512) DEFAULT NULL,

  maybe_audio_mime_type VARCHAR(32) DEFAULT NULL,

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  -- If no user is logged in, this is null.
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  creator_set_visibility ENUM(
      'public',
      'hidden',
      'private'
  ) NOT NULL DEFAULT 'public',

  -- Flags to disable branding
  disable_end_bump BOOL NOT NULL DEFAULT false,
  disable_watermark BOOL NOT NULL DEFAULT false,

  -- ========== JOB SYSTEM DETAILS ==========

  -- Jobs begin as "pending", then transition to other states.
  --
  --  * Pending = job is ready to go
  --  * Started = job is running
  --  * Complete_Success = job is done (success)
  --  * Complete_Failure = job is done (failure)
  --  * Attempt_Failed = job failed but may retry.
  --  * Dead = job failed permanently.
  --
  -- Pending -> Started -> Complete_Success
  --                    |-> Complete_Failure
  --                    \-> Attempt_Failed -> Started -> { Complete, Failed, Dead }
  status ENUM(
      'pending',
      'started',
      'complete_success',
      'complete_failure',
      'attempt_failed',
      'dead') NOT NULL DEFAULT 'pending',

  -- We can track this against a "max_attempt_count"
  attempt_count INT(3) NOT NULL DEFAULT 0,

  -- If there is a failure, tell the user why.
  failure_reason VARCHAR(512) DEFAULT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- Failed jobs will set a next attempt time.
  -- Subsequent tries can increase the timeout.
  -- Failures because of permissions require human intervention => [retry_at=null].
  -- Failures because of invalid files are dead => [status=dead].
  retry_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),
  KEY fk_on_success_result_token (on_success_result_token),
  KEY fk_maybe_w2l_template_token (maybe_w2l_template_token),
  KEY fk_maybe_tts_inference_result_token (maybe_tts_inference_result_token),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY index_status (status),
  KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
