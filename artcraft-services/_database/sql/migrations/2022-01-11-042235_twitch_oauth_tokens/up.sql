-- noinspection SqlDialectInspectionForFile
-- noinspection SqlNoDataSourceInspectionForFile
-- noinspection SqlResolveForFile

-- **NOTE ABOUT DESIGN**:
--   This table contains _many_ OAuth tokens for any single user. (many users : many tokens)
--   An entire history of tokens is kept in the table.
--   Apart from "deleted_at", etc., these records should largely be IMMUTABLE.

CREATE TABLE twitch_oauth_tokens(
  -- Not used for anything except replication.
  id BIGINT(20) UNSIGNED NOT NULL AUTO_INCREMENT,

  -- Minted on first request and copied on each refresh.
  -- This allows us to group access tokens all the way back up the
  -- refresh chain.
  oauth_refresh_grouping_token VARCHAR(32) NOT NULL,

  -- ========== STORYTELLER/FAKEYOU USER ==========

  -- Foreign key to user (Storyteller/FakeYou user)
  -- If no user is associated (anonymous Twitch user), then this is null.
  maybe_user_token VARCHAR(32) DEFAULT NULL,

  -- ========== TWITCH USER ==========

  -- The user ID / channel ID are the same
  -- https://discuss.dev.twitch.tv/t/what-is-the-difference-between-the-stream--id-and-channel--id/4423
  -- Several suggest this should be a string and not an integer
  -- https://discuss.dev.twitch.tv/t/type-of-user-id-in-api-responses/10205
  -- Yep, strings
  -- https://discuss.dev.twitch.tv/t/bug-v5-api-returns--id-as-string-for-featured-channels/10310
  twitch_user_id VARCHAR(64) NOT NULL,

  -- Twitch usernames are between 4 and 25 characters.
  -- This is returned in the oauth flow
  twitch_username VARCHAR(32) NOT NULL,

  -- Twitch username, but lowercase for lookup.
  twitch_username_lowercase VARCHAR(32) NOT NULL,

  -- ========== OAUTH TOKEN DETAILS ==========

  -- The ever important OAuth access token.
  access_token VARCHAR(128) NOT NULL,

  -- If we can refresh the token, this is the thing to use.
  maybe_refresh_token VARCHAR(128) DEFAULT NULL,

  -- Should be "bearer". Included in the OAuth redemption.
  token_type VARCHAR(32) DEFAULT NULL,

  -- When the token expires from time of first grant.
  -- (We don't update this field.)
  -- Null if it does not expire or we were not informed
  expires_in_seconds INT(10) UNSIGNED DEFAULT NULL,

  -- Number of times this token has been refreshed
  -- The original token creation will be "0", and each
  -- subsequent time will bump by one. Mostly for debugging.
  refresh_count INT(10) UNSIGNED DEFAULT 0,

  -- ======================================================
  -- ==================== OAUTH SCOPES ====================
  -- ======================================================

  -- TODO: Probably could have stored this in a bitfield. Ugh.

  -- "analytics:read:extensions"
  -- "View analytics data for the Twitch Extensions owned by the authenticated account."
  has_analytics_read_extensions BOOLEAN NOT NULL DEFAULT FALSE,

  -- scope: "analytics:read:games"
  -- "View analytics data for the games owned by the authenticated account."
  has_analytics_read_games BOOLEAN NOT NULL DEFAULT FALSE,

  -- "bits:read"
  -- "View Bits information for a channel."
  has_bits_read BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:edit:commercial"
  -- "Run commercials on a channel."
  has_channel_edit_commercial BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:broadcast"
  -- "Manage a channel's broadcast configuration, including updating channel configuration and managing stream markers and stream tags."
  has_channel_manage_broadcast  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:extensions"
  -- "Manage a channel's Extension configuration, including activating Extensions."
  has_channel_manage_extensions  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:polls"
  -- "Manage a channel's polls."
  has_channel_manage_polls  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:predictions"
  -- "Manage of channel's Channel Points Predictions"
  has_channel_manage_predictions  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:redemptions"
  -- "Manage Channel Points custom rewards and their redemptions on a channel."
  has_channel_manage_redemptions  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:schedule"
  -- "Manage a channel's stream schedule."
  has_channel_manage_schedule  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:manage:videos"
  -- "Manage a channel's videos, including deleting videos."
  has_channel_manage_videos  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:moderate"
  -- "Perform moderation actions in a channel. The user requesting the scope must be a moderator in the channel."
  has_channel_moderate  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:editors"
  -- "View a list of users with the editor role for a channel."
  has_channel_read_editors  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:goals"
  -- "View Creator Goals for a channel."
  has_channel_read_goals  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:hype_train"
  -- "View Hype Train information for a channel."
  has_channel_read_hype_train  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:polls"
  -- "View a channel's polls."
  has_channel_read_polls  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:predictions"
  -- "View a channel's Channel Points Predictions."
  has_channel_read_predictions  BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:redemptions"
  -- "View Channel Points custom rewards and their redemptions on a channel."
  -- eg. PubSub subscribe to `channel-points-channel-v1.<channel_id>` (channel points spends)
  has_channel_read_redemptions BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:stream_key"
  -- "View an authorized user's stream key."
  has_channel_read_stream_key BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel:read:subscriptions"
  -- "View a list of all subscribers to a channel and check if a user is subscribed to a channel."
  -- eg. Enumerate list of subscribers for a channel
  -- eg. PubSub subscribe to `channel-subscribe-events-v1.<channel ID>` (subscribe, resubscribe, gift)
  has_channel_read_subscriptions BOOLEAN NOT NULL DEFAULT FALSE,

  -- "channel_subscriptions"
  -- "\\[DEPRECATED\\] Read all subscribers to your channel."
  has_channel_subscriptions BOOLEAN NOT NULL DEFAULT FALSE,

  -- "chat:edit"
  -- "Send live stream chat and rooms messages."
  has_chat_edit BOOLEAN NOT NULL DEFAULT FALSE,

  -- "chat:read"
  -- "View live stream chat and rooms messages."
  has_chat_read BOOLEAN NOT NULL DEFAULT FALSE,

  -- "clips:edit"
  -- "Manage Clips for a channel."
  has_clips_edit BOOLEAN NOT NULL DEFAULT FALSE,

  -- "moderation:read"
  -- "View a channel's moderation data including Moderators, Bans, Timeouts, and Automod settings."
  has_moderation_read BOOLEAN NOT NULL DEFAULT FALSE,

  -- "moderator:manage:automod"
  -- "Manage messages held for review by AutoMod in channels where you are a moderator."
  has_moderator_manage_auto_mod BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:edit"
  -- "Manage a user object."
  has_user_edit BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:edit:broadcast"
  -- "Edit your channel's broadcast configuration, including extension configuration. (This scope implies user:read:broadcast capability.)"
  has_user_edit_broadcast BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:edit:follows"
  -- "Edit a user's follows."
  has_user_edit_follows BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:manage:blocked_users"
  -- "Manage the block list of a user."
  has_user_manage_blocked_users BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:read:blocked_users"
  -- "View the block list of a user."
  has_user_read_blocked_users BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:read:broadcast"
  -- "View a user's broadcasting configuration, including Extension configurations."
  has_user_read_broadcast BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:read:email"
  -- "Read an authorized user's email address."
  has_user_read_email BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:read:follows"
  -- "View the list of channels a user follows."
  -- eg. We can use this to see if a Twitch user follows us.
  has_user_read_follows BOOLEAN NOT NULL DEFAULT FALSE,

  -- "user:read:subscriptions"
  -- "View if an authorized user is subscribed to specific channels."
  has_user_read_subscriptions BOOLEAN NOT NULL DEFAULT FALSE,

  -- "whispers:edit"
  -- "Send whisper messages."
  has_whispers_edit BOOLEAN NOT NULL DEFAULT FALSE,

  -- "whispers:read"
  -- "View your whisper messages."
  has_whispers_read BOOLEAN NOT NULL DEFAULT FALSE,

  -- ========== SECURITY ==========

  -- For abuse tracking.
  -- Wide enough for IPv4/6
  ip_address_creation VARCHAR(40) DEFAULT NULL,

  -- ========== VECTOR CLOCK ==========

  -- Incremented with every update.
  version INT UNSIGNED NOT NULL DEFAULT 0,

  -- ========== TIMESTAMPS ==========

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,

  -- This is when the OAuth token will expire.
  -- While we need to refresh before this time, Twitch recommends against eager
  -- refreshes and instead wants the code to lazily refresh.
  -- This also has no bearing on the "refresh_token".
  -- We'll just use this for bookkeeping.
  expires_at TIMESTAMP DEFAULT NULL,

  -- If the OAuth token is deleted, we set this.
  -- We'll need to set it for all of a user's OAuth tokens since this table stores many tokens.
  user_deleted_at TIMESTAMP NULL,
  mod_deleted_at TIMESTAMP NULL,

  -- INDICES --
  PRIMARY KEY (id),
  KEY index_oauth_refresh_grouping_token (oauth_refresh_grouping_token),
  KEY fk_maybe_user_token (maybe_user_token),
  KEY index_twitch_user_id (twitch_user_id),
  -- KEY index_twitch_username (twitch_username),
  KEY index_twitch_username_lowercase (twitch_username_lowercase),
  KEY index_expires_at (expires_at),
  KEY index_user_deleted_at (user_deleted_at),
  KEY index_mod_deleted_at (mod_deleted_at)

  -- KEY index_has_bits_read (has_bits_read),
  -- KEY index_creator_ip_address (creator_ip_address)

) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin;

