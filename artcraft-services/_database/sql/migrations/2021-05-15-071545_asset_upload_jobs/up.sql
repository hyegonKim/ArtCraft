-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: See the "squashed" copy of this for better documentation
-- and the current state of the table.

-- TTS MODELS
CREATE TABLE tts_model_upload_jobs (
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- This is so the in-progress results can be looked up by the UI.
  token VARCHAR(32) NOT NULL,

  -- Idempotency token
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== SUCCESS CASE ==========

  -- If the job completes successfully, this is the result token.
  -- This is only populated on a successful result.
  on_success_result_token VARCHAR(32) DEFAULT NULL,

  -- ========== UPLOAD DETAILS ==========

  -- The name of the voice
  -- The "name" of the voice model, which might be complicated.
  -- If maybe_subject_token (etc.) is set, then it's authoritative instead.
  title VARCHAR(255) NOT NULL,

  -- NB: THIS ENUM IS OUT OF DATE!
  -- Check the squashed sql file as an updated reference!
  -- NB: DO NOT CHANGE ORDER; APPEND ONLY!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  tts_model_type ENUM(
      'not-set',
      'tacotron2',
      'glowtts',
      'glowtts-vocodes'
  ) NOT NULL DEFAULT 'not-set',

  -- Can be linked to a well-known voice
  -- maybe_subject_token VARCHAR(32) DEFAULT NULL,
  -- maybe_actor_subject_token VARCHAR(32) DEFAULT NULL,

  -- If we need to download the file from Google Drive.
  download_url VARCHAR(512) DEFAULT NULL,

  -- NB: DO NOT SORT!
  -- download_url_type ENUM(
  --     'google-drive',
  --     'web'
  -- ) NOT NULL DEFAULT 'web',

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  creator_user_token VARCHAR(32) NOT NULL,

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
  KEY fk_creator_user_token (creator_user_token),
  KEY index_status (status),
  KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

-- WAV2LIP TEMPLATES
CREATE TABLE w2l_template_upload_jobs (
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- This is so the in-progress results can be looked up by the UI.
  token VARCHAR(32) NOT NULL,

  -- Idempotency token
  -- This is so the frontend client doesn't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== SUCCESS CASE ==========

  -- If the job completes successfully, this is the result token.
  -- This is only populated on a successful result.
  on_success_result_token VARCHAR(32) DEFAULT NULL,

  -- ========== UPLOAD DETAILS ==========

  -- The title of the template
  title VARCHAR(255) NOT NULL,

  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  template_type ENUM(
    'unknown',
    'video',
    'image'
  ) NOT NULL DEFAULT 'unknown',

  -- Can be linked to a well-known subject
  -- maybe_subject_token VARCHAR(32) DEFAULT NULL,
  -- maybe_actor_subject_token VARCHAR(32) DEFAULT NULL,

  -- If we need to download the file from Google Drive.
  download_url VARCHAR(512) DEFAULT NULL,

  -- NB: DO NOT SORT!
  -- download_url_type ENUM(
  --     'google-drive',
  --     'web'
  -- ) NOT NULL DEFAULT 'web',

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  creator_user_token VARCHAR(32) NOT NULL,

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
  KEY fk_creator_user_token (creator_user_token),
  KEY index_status (status),
  KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
