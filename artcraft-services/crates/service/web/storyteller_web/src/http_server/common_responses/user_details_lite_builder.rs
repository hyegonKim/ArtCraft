use artcraft_api_defs::common::responses::user_details_light::{UserDefaultAvatarInfo, UserDetailsLight};
use tokens::tokens::users::UserToken;
use utoipa::ToSchema;

use crate::http_server::common_responses::user_avatars::default_avatar_color_from_username::default_avatar_color_from_username;
use crate::http_server::common_responses::user_avatars::default_avatar_from_username::default_avatar_from_username;

pub struct UserDetailsLightBuilder {}

impl UserDetailsLightBuilder {

  pub fn from_db_fields(
    user_token: &UserToken,
    username: &str,
    display_name: &str,
    gravatar_hash: &str,
  ) -> UserDetailsLight {
    UserDetailsLight{
      default_avatar: UserDefaultAvatarInfoBuilder::from_username(&username),
      user_token: user_token.clone(),
      username: username.to_string(),
      display_name: display_name.to_string(),
      gravatar_hash: gravatar_hash.to_string(),
    }
  }

  pub fn from_optional_db_fields(
    maybe_user_token: Option<&UserToken>,
    maybe_username: Option<&str>,
    maybe_display_name: Option<&str>,
    maybe_gravatar_hash: Option<&str>,
  ) -> Option<UserDetailsLight> {
    Self::from_optional_db_fields_owned(
      maybe_user_token.map(|u| u.clone()),
      maybe_username.map(|s|s.to_string()),
      maybe_display_name.map(|s|s.to_string()),
      maybe_gravatar_hash.map(|s|s.to_string()),
    )
  }

  pub fn from_optional_db_fields_owned(
    maybe_user_token: Option<UserToken>,
    maybe_username: Option<String>,
    maybe_display_name: Option<String>,
    maybe_gravatar_hash: Option<String>,
  ) -> Option<UserDetailsLight> {
    match (maybe_user_token, maybe_username, maybe_display_name, maybe_gravatar_hash) {
      (Some(user_token), Some(username), Some(display_name), Some(gravatar_hash)) => {
        Some(UserDetailsLight {
          default_avatar: UserDefaultAvatarInfoBuilder::from_username(&username),
          user_token,
          username,
          display_name,
          gravatar_hash,
        })
      }
      _ => {
        None
      }
    }
  }
}

pub struct UserDefaultAvatarInfoBuilder {}

impl UserDefaultAvatarInfoBuilder {
  /// Default avatars are based on username, not user token.
  /// NB(bt,2023-08-23): I think the thinking here was that we'd always have the
  /// username on hand and that it was more entropic.
  pub fn from_username(username: &str) -> UserDefaultAvatarInfo {
    UserDefaultAvatarInfo {
      image_index: default_avatar_from_username(username),
      color_index: default_avatar_color_from_username(username),
    }
  }
}

#[cfg(test)]
mod tests {
  use tokens::tokens::users::UserToken;

  use crate::http_server::common_responses::user_details_lite_builder::UserDetailsLightBuilder;

  #[test]
  fn test_from_optional_db_fields() {
    let user_token = UserToken::new_from_str("token");
    let username = "username";
    let display_name = "display_name";
    let gravatar_hash= "adsf";

    let user_details = UserDetailsLightBuilder::from_optional_db_fields(
      Some(&user_token),
      Some(username),
      Some(display_name),
      Some(gravatar_hash)
    );

    let user_details = user_details.expect("Should not be optional.");

    assert_eq!(user_details.user_token, user_token);
    assert_eq!(user_details.username, username);
    assert_eq!(user_details.display_name, display_name);
    assert_eq!(user_details.gravatar_hash, gravatar_hash);
    assert_eq!(user_details.default_avatar.color_index, 5);
    assert_eq!(user_details.default_avatar.image_index, 16);
  }
}
