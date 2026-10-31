use time::Duration;

// TODO: Think about these timings more.

/// Our Twitch PubSub subscriber threads must renew leases in
/// order to be granted an exclusive "lock" on a single user.
/// If the underlying Redis key goes away, another thread can
/// begin leasing.
pub const LEASE_TIMEOUT_SECONDS : usize = 60;

/// The amount of time to wait before renewing our lease.
pub const LEASE_RENEW_PERIOD : Duration = Duration::seconds(30);

/// The amount of time to wait before checking or lease is valid.
pub const LEASE_CHECK_PERIOD : Duration = Duration::seconds(15);

/// The OBS websocket polls a key to inform the backend that the
/// user is still active.
pub const OBS_ACTIVE_TTL_SECONDS : usize = 120;

/// How often to check the OBS session activity.
pub const OBS_ACTIVE_CHECK_PERIOD : Duration = Duration::seconds(30);

/// When to expire the streamer-specific TTS job queue notifications
pub const STREAMER_TTS_JOB_QUEUE_TTL_SECONDS : usize = 600;
