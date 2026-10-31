-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE generic_download_jobs (
  -- Not used for anything except replication.
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
  on_success_downloaded_entity_token VARCHAR(32) DEFAULT NULL,

  -- The type of the object will vary based on the type of the upload,
  -- and we may include heuristics that auto-detect types in the future
  -- Example types: 'hifigan', 'hifigan_rocket_vc' (codename for softvc), 'tacotron2', ...
  on_success_downloaded_entity_type VARCHAR(32) DEFAULT NULL,

  -- ========== UPLOAD DETAILS ==========

  -- A "well defined" type of upload
  -- Examples: "hifigan", "tacotron2", etc.
  download_type VARCHAR(32) NOT NULL,

  -- Assume the same URL can be uploaded more than once (in case of errors or
  -- people sharing models)

  download_url VARCHAR(512) NOT NULL,

  -- The user-supplied title of the upload
  -- This might have different meanings for different upload types
  -- The "name" of the voice model, which might be complicated.
  -- If maybe_subject_token (etc.) is set, then it's authoritative instead.
  title VARCHAR(255) NOT NULL,

  -- ========== CREATOR DETAILS AND PREFERENCES ==========

  -- Foreign key to user
  creator_user_token VARCHAR(32) NOT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  creator_ip_address VARCHAR(40) NOT NULL,

  -- The creator can set a desired visibility for their data.
  -- This does not always apply to every upload type.
  -- Additionally, some upload types may require moderator approval prior
  -- to being publicly listed, and this field has no bearing on that.
  -- NB: DO NOT CHANGE THE ORDER OF THE ENUM VALUES DURING SCHEMA MIGRATIONS.
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
  KEY fk_on_download_type (download_type),
  KEY fk_creator_user_token (creator_user_token),
  KEY index_status (status),
  KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
