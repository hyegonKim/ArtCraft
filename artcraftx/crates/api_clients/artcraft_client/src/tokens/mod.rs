//! Strongly-typed identifier tokens from the Artcraft/Storyteller API
//! (Stripe-like prefixes, e.g. `user_`, `jinf_`).
//!
//! These are OPAQUE string wrappers: the server mints them, the client only
//! carries them around. There is deliberately no token generation here.

/// Implement the standard client-side token surface for a string wrapper
/// type: constructors from server-provided strings, accessors, and `Display`.
macro_rules! impl_client_token {
  ($t:ident) => {
    impl $t {
      #[inline]
      pub fn new(value: String) -> Self {
        $t(value)
      }

      #[inline]
      pub fn new_from_str(value: &str) -> Self {
        $t(value.to_string())
      }

      #[inline]
      pub fn as_str(&self) -> &str {
        &self.0
      }
    }

    impl std::fmt::Display for $t {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
      }
    }
  }
}

pub mod api_keys;
pub mod app_session;
pub mod characters;
pub mod folders;
pub mod generic_inference_jobs;
pub mod model_weights;
pub mod tags;
pub mod uploaded_video_notes;
pub mod uploaded_videos;
pub mod user_referral_codes;
pub mod user_spend_events;
pub mod user_subscriptions;
pub mod users;
pub mod wallet_ledger_entries;
pub mod wallets;

// Non-unique tokens (used as indices, not primary keys)
pub mod non_unique;
