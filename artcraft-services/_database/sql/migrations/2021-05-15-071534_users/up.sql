-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- NB: See the "squashed" copy of this for better documentation
-- and the current state of the table.

CREATE TABLE users (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Visible "primary key"
  token VARCHAR(32) NOT NULL,

  -- Username is a lookup key; display_name allows the user to add custom case.
  username VARCHAR(20) NOT NULL,
  display_name VARCHAR(20) NOT NULL,

  email_address VARCHAR(255) NOT NULL,
  email_confirmed BOOLEAN NOT NULL DEFAULT false,

  -- The role assigned to the user confers permissions.
  user_role_slug VARCHAR(16) NOT NULL,

  -- ========== CREDENTIALS ==========

  -- Bcrypt password hash. Granted, there are newer methods:
  -- https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
  -- Passwords may be a max of 64 characters per BCrypt.
  password_hash BINARY(60) NOT NULL,

  -- Incremented with every update to the password.
  password_version INT NOT NULL DEFAULT 0,

  -- ========== ABUSE TRACKING ==========

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,
  ip_address_last_login VARCHAR(40) NOT NULL,
  ip_address_last_update VARCHAR(40) NOT NULL,

  -- ========== DISPLAY / PROFILE ==========

  -- Gravatar image hashes are precomputed.
  email_gravatar_hash CHAR(32) NOT NULL,

  -- The profile of the user in markdown (user editable).
  profile_markdown TEXT NOT NULL,

  -- Generated HTML (not user-editable).
  profile_rendered_html TEXT NOT NULL,

  -- An uploaded avatar. Public hash in our bucket.
  -- avatar_public_bucket_hash CHAR(32) DEFAULT NULL,

  -- ========== SOCIAL MEDIA ==========

  -- Social media usernames
  -- These are not confirmed. We'll need to build an OAuth system to handle that.
  discord_username VARCHAR(36) DEFAULT NULL,
  twitter_username VARCHAR(36) DEFAULT NULL,
  twitch_username VARCHAR(36) DEFAULT NULL,
  patreon_username VARCHAR(36) DEFAULT NULL,
  github_username VARCHAR(36) DEFAULT NULL,
  cashapp_username VARCHAR(36) DEFAULT NULL,
  website_url VARCHAR(255) DEFAULT NULL,

  -- ========== USER PREFERENCES ==========

  -- If the user doesn't want to use gravatar and doesn't have an uploaded avatar.
  disable_gravatar BOOLEAN NOT NULL DEFAULT false,

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  -- Visibility preference used at TTS inference time (which can be changed on the result itself).
  -- Moderators will still see them.
  preferred_tts_result_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- (THIS MIGHT NOT BE USED)
  -- NB: DO NOT SORT!
  -- THIS MUST MATCH THE RESPECTIVE JOBS TABLE.
  -- Visibility preference used at TTS inference time (which can be changed on the result itself).
  -- Moderators will still see them.
  preferred_w2l_result_visibility ENUM(
    'public',
    'hidden',
    'private'
  ) NOT NULL DEFAULT 'public',

  -- Auto play preferences
  -- Tri-state because we want to potentially change the default behavior.
  auto_play_audio_preference BOOLEAN DEFAULT NULL,
  auto_play_video_preference BOOLEAN DEFAULT NULL,

  -- Favorite TTS voice to use by default
  -- maybe_preferred_tts_model_token VARCHAR(32) DEFAULT NULL,

  -- Favorite W2L model to use by default
  -- maybe_preferred_w2l_template_token VARCHAR(32) DEFAULT NULL,

  -- Settings
  -- DO NOT REORDER.
  -- dark_mode_preference ENUM(
  --   'light-mode',
  --   'dark-mode',
  --   'use-clock'
  --  ) NOT NULL DEFAULT 'light-mode',

  -- ========== STATS ==========

  -- For tracking stats.
  -- The "cached" values are updated by a background job.
  -- cached_tts_rendered_counter INT(10) NOT NULL DEFAULT 0,
  -- cached_w2l_rendered_counter INT(10) NOT NULL DEFAULT 0,

  -- ========== MODERATION DETAILS ==========

  -- Different than deleted.
  -- Users still show up, but can't do anything.
  is_banned BOOLEAN NOT NULL DEFAULT false,

  -- If a moderator has comments.
  maybe_mod_comments VARCHAR(255) DEFAULT NULL,

  -- The last moderator that made changes.
  maybe_mod_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- If the user is deleted, we set this.
  -- This is different than banned. These users won't show up at all.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  UNIQUE KEY (username),
  UNIQUE KEY (email_address),
  KEY fk_user_role_slug (user_role_slug)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE user_roles (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Effective "primary key"
  slug CHAR(16) NOT NULL,

  name VARCHAR(255) NOT NULL,

  -- Usage
  can_use_tts BOOLEAN NOT NULL DEFAULT FALSE,
  can_use_w2l BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_own_tts_results BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_own_w2l_results BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_own_account BOOLEAN NOT NULL DEFAULT FALSE,

  -- Contribution
  can_upload_tts_models BOOLEAN NOT NULL DEFAULT FALSE,
  can_upload_w2l_templates BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_own_tts_models BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_own_w2l_templates BOOLEAN NOT NULL DEFAULT FALSE,

  -- Moderation
  can_approve_w2l_templates BOOLEAN NOT NULL DEFAULT FALSE,
  -- Edit includes "lock_from_use" and "lock_from_modification"
  can_edit_other_users_profiles BOOLEAN NOT NULL DEFAULT FALSE,
  can_edit_other_users_tts_models BOOLEAN NOT NULL DEFAULT FALSE,
  can_edit_other_users_w2l_templates BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_other_users_tts_models BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_other_users_tts_results BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_other_users_w2l_templates BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_other_users_w2l_results BOOLEAN NOT NULL DEFAULT FALSE,
  can_ban_users BOOLEAN NOT NULL DEFAULT FALSE,
  can_delete_users BOOLEAN NOT NULL DEFAULT FALSE,

  -- Incremented with every update.
  version INT NOT NULL DEFAULT 0,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (slug)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

CREATE TABLE user_sessions (
  -- Not used for anything except replication.
  id BIGINT(20) NOT NULL AUTO_INCREMENT,

  -- Session entropy
  token VARCHAR(32) NOT NULL,

  -- Foreign key to user
  user_token VARCHAR(32) NOT NULL,

  -- Track each session's creation IP.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- Session termination time.
  -- This must be set by the server code, or the session is invalid
  expires_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- deletion = session termination
  -- Typically these are destroyed by users, but if in the future we allow mods to
  -- delete them, it doesn't really matter who did the deletion: sessions are not
  -- designed to be recoverable.
  deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  UNIQUE KEY (token),
  KEY fk_user_token (user_token),
  KEY index_ip_address_creation (ip_address_creation)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;
