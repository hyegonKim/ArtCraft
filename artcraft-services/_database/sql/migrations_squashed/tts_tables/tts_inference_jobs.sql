-- NB: This is a manually squashed view of all the CREATE and ALTER statements,
-- with comments attached to the fields for centralized documentation.

-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

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

  -- If set, use a different pretrained vocoder.
  maybe_override_pretrained_vocoder VARCHAR(64) DEFAULT NULL,

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

  -- ========== SOURCE METADATA ==========

  is_from_api BOOLEAN NOT NULL DEFAULT FALSE,
  is_for_twitch BOOLEAN NOT NULL DEFAULT FALSE,

  -- If true, the request gets routed to a special "debug" worker.
  -- NB: This has a *DIFFERENT* meaning on the generic_inference_jobs table.
  is_debug_request BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== PREMIUM FEATURES METADATA ==========

  -- The maximum duration for generated audio in seconds.
  -- Zero is implied to be the default value, which is typically 12 seconds.
  -- A negative value implies "unlimited".
  max_duration_seconds INTEGER NOT NULL DEFAULT 0,

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

  -- Priority *increases*, so a level of 2 will be higher than 1.
  -- These are the level semantics currently:
  --   - All jobs from anonymous FakeYou users have level 0.
  --   - All jobs from logged in FakeYou users have level 1.
  --   - All jobs from Twitch TTS (unpaid) have level 10 (ten).
  --   - (There will be future levels for paid Twitch and social FakeYou rewards.)
  priority_level TINYINT UNSIGNED NOT NULL DEFAULT 0,

  -- We can track this against a "max_attempt_count"
  attempt_count INT(3) NOT NULL DEFAULT 0,

  -- If there is a failure, tell the user why.
  -- This is user-facing, so keep it sanitized and reasonable.
  failure_reason VARCHAR(512) DEFAULT NULL,

  -- Optional internal-only debugging information in the case of failure.
  internal_debugging_failure_reason VARCHAR(512) DEFAULT NULL,

  -- The last worker (hostname or pod name) to touch the job, either in the case of success or failure.
  last_assigned_worker VARCHAR(255) DEFAULT NULL,

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
  KEY index_creator_ip_address (creator_ip_address),
  KEY index_is_debug_request (is_debug_request)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
