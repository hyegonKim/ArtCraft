use stripe_shared::CheckoutSession;
use tokens::tokens::user_sessions::UserSessionToken;
use tokens::tokens::users::UserToken;

pub (super) struct CreationPayload {
  pub checkout_session: CheckoutSession,
  pub maybe_new_user_metadata: Option<UserMetadata>
}

pub (super) struct UserMetadata {
  pub user_token: UserToken,
  pub session_token: UserSessionToken,
  pub username: String,
  pub display_name: String,
  pub email_address: String,
}
