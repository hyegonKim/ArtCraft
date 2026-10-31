use crate::enums::common::payments_namespace::PaymentsNamespace;
use serde_derive::{Deserialize, Serialize};
use crate::tokens::users::UserToken;
use crate::tokens::wallets::WalletToken;

pub const MODERATOR_CREATE_WALLET_FOR_USER_PATH: &str = "/v1/moderation/wallet/create_for_user";

#[derive(Deserialize)]
pub struct ModeratorCreateWalletForUserRequest {
  pub user_token: Option<UserToken>,
  pub payments_namespace: Option<PaymentsNamespace>,
}

#[derive(Serialize)]
pub struct ModeratorCreateWalletForUserResponse {
  pub success: bool,
  pub wallet_token: WalletToken,
}
