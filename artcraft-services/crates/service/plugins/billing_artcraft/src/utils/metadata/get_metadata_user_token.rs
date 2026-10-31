use crate::configs::stripe_artcraft_metadata_keys::STRIPE_ARTCRAFT_METADATA_USER_TOKEN;
use tokens::tokens::users::UserToken;

pub (crate) fn get_metadata_user_token(
  metadata: &std::collections::HashMap<String, String>,
) -> Option<UserToken> {
  metadata.get(STRIPE_ARTCRAFT_METADATA_USER_TOKEN)
      .map(|t| UserToken::new_from_str(&t))
}
