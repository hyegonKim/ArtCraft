use stripe::RequestStrategy;

/// Retry three times with backoff.
pub const STRIPE_CLIENT_RETRY_STRATEGY : RequestStrategy = RequestStrategy::ExponentialBackoff(3);
