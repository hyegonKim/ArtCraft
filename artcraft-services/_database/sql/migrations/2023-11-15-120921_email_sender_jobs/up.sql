-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

CREATE TABLE email_sender_jobs (

  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key" (PUBLIC)
  -- This is so the in-progress results can be looked up by the UI.
  token VARCHAR(32) NOT NULL,

  -- Idempotency token from client
  -- This is so we don't submit duplicate jobs.
  uuid_idempotency_token VARCHAR(36) NOT NULL,

  -- ========== EMAIL TYPE AND ARGS ==========

  -- Type of email (and the template) to use.
  -- This is the most important part of the email job.
  -- Possible values:
  --  * password_reset
  --  * welcome
  email_category VARCHAR(32) NOT NULL,

  -- Polymorphic arguments payload that depends on the type of email job.
  -- TEXT = 65,535 bytes (64 KiB), ~= 4 bytes per UTF-8 character, ~ 16383 characters.
  maybe_email_args TEXT DEFAULT NULL,

  -- ========== EMAIL DESTINATION ==========

  -- Email we're sending to (always authoritative over the user token
  -- record, even if user token is set)
  destination_email_address VARCHAR(255) DEFAULT NULL,

  -- Maybe the user record this is being sent to.
  maybe_destination_user_token  VARCHAR(32) DEFAULT NULL,

  -- ========== EMAIL LANGUAGE ==========

  -- Email the language should be sent in.
  -- The full IETF BCP47 language tag (eg. en, en-US, es-419, ja-JP, pt, etc.)
  -- The wide varchar space is because these language tags can become cumbersome, eg "hy-Latn-IT-arevela"
  ietf_language_tag VARCHAR(64) NOT NULL DEFAULT 'en',

  -- Email the language should be sent in.
  -- The IETF BCP47 language tag's primary language subtag (eg. "es-419" becomes "es" and "en-US" becomes "en")
  ietf_primary_language_subtag VARCHAR(12) NOT NULL DEFAULT 'en',

  -- ========== CREATOR DETAILS ==========

  -- Foreign key to user that triggered the job (eg. for user DMs)
  maybe_creator_user_token VARCHAR(32) DEFAULT NULL,

  -- AVT of the user that triggered the job (eg. for user DMs)
  -- Based on a cookie sent by the frontend.
  -- We'll save this even if the user is logged in.
  maybe_creator_anonymous_visitor_token VARCHAR(32) DEFAULT NULL,

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  maybe_creator_ip_address VARCHAR(40) DEFAULT NULL,

  -- ========== WORKLOAD SOURCE AND PRIORITY ==========

  -- Priority *increases*, so a level of 2 will be higher than 1.
  -- These are the level semantics currently:
  --   - (N/A; all emails should be roughly the same importance for now)
  priority_level SMALLINT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== DEVELOPMENT AND DEBUGGING METADATA ==========

  -- If true, the request gets a "debug" flag, which may do different
  -- things depending on the type of work. This doesn't really have any
  -- specific meaning -- it can be used to flag for anything -- but should
  -- typically be used for one-off debugging and not a part of the normal
  -- product surface area. Contrast this with `maybe_routing_tag`, which
  -- forces the job onto a specific physical server: this might be a flag
  -- deployed to handle debugging across the entire fleet.
  is_debug_request BOOLEAN NOT NULL DEFAULT FALSE,

  -- If set, the request gets processed by a special "tagged" worker
  -- that matches this tag. The ordinary workers will ignore this work.
  -- The tag is typically added via a special HTTP header when the work
  -- is enqueued. In practice, this means we can canary deploy or cordon
  -- off special workers to handle certain jobs.
  maybe_routing_tag VARCHAR(32) DEFAULT NULL,

  -- ========== JOB SYSTEM DETAILS ==========

  -- Jobs begin as "pending", then transition to other states.
  --
  --  * Pending = job is ready to go
  --  * Started = job is running
  --  * Complete_Success = job is done (success)
  --  * Complete_Failure = job is done (permanent failure, eg. a Face Animator job without a face)
  --  * Attempt_Failed = job failed but may retry.
  --  * Dead = job failed permanently (ie. exhausted retries)
  --  * Cancelled_By_User = the user canceled their own job
  --  * Cancelled_By_System = the system terminated the job (ie. load shedding, user account deletion, etc.)
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
    'dead',
    'cancelled_by_user',
    'cancelled_by_system'
  ) NOT NULL DEFAULT 'pending',

  -- We can track this against a "max_attempt_count"
  attempt_count SMALLINT UNSIGNED NOT NULL DEFAULT 0,

  -- Optional internal-only debugging information in the case of failure.
  internal_debugging_failure_reason VARCHAR(512) DEFAULT NULL,

  -- Worker hostname (linux hostname, k8s pod name)
  -- Assigned when a worker picks up the job
  -- Reassigned if the job fails and gets picked up again
  assigned_worker VARCHAR(128) DEFAULT NULL,

  -- Cluster name (k8s)
  -- Assigned when a worker picks up the job
  -- Reassigned if the job fails and gets picked up again
  assigned_cluster VARCHAR(128) DEFAULT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- Failed jobs will set a next attempt time.
  -- Subsequent tries can increase the timeout.
  -- Failures because of permissions require human intervention => [retry_at=null].
  -- Failures because of invalid files are dead => [status=dead].
  retry_at TIMESTAMP NULL,

  -- Set when the job first starts executing
  first_started_at TIMESTAMP NULL,

  -- Set when the job is successfully completed
  successfully_completed_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (uuid_idempotency_token),
  KEY index_email_category (email_category),
  KEY fk_maybe_creator_user_token (maybe_creator_user_token),
  KEY index_maybe_creator_ip_address (maybe_creator_ip_address),
  KEY index_priority_level (priority_level),
  KEY index_is_debug_request (is_debug_request),
  KEY index_maybe_routing_tag (maybe_routing_tag),
  KEY index_status (status)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
