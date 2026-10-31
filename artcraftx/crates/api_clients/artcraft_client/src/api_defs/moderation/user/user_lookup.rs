use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use crate::tokens::users::UserToken;

pub const MODERATOR_USER_LOOKUP_PATH: &str = "/v1/moderation/users/lookup";

#[derive(Deserialize)]
pub struct ModeratorUserLookupRequest {
  pub search: String,
}

#[derive(Serialize)]
pub struct ModeratorUserLookupSuccessResponse {
  pub success: bool,
  pub maybe_user: Option<ModeratorUserLookupUserDetails>,
}

#[derive(Serialize)]
pub struct ModeratorUserLookupUserDetails {
  pub token: UserToken,
  pub username: String,
  pub display_name: String,
  pub username_is_generated: bool,
  pub is_temporary: bool,
  pub username_is_not_customized: bool,
  pub email_address: String,
  pub email_confirmed: bool,
  pub email_is_synthetic: bool,
  pub is_without_password: bool,
  pub ip_address_creation: String,
  pub ip_address_last_login: String,
  pub maybe_avatar_media_file_token: Option<MediaFileToken>,
  pub email_gravatar_hash: String,
  pub is_banned: bool,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}
