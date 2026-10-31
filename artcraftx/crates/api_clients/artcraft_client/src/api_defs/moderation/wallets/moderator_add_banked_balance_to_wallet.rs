use serde_derive::{Deserialize, Serialize};
use crate::tokens::wallets::WalletToken;

pub const MODERATOR_ADD_BANKED_BALANCE_TO_WALLET_PATH: &str = "/v1/moderation/wallet/{wallet_token}/add_banked_balance";

#[derive(Deserialize)]
pub struct ModeratorAddBankedBalanceToWalletPathInfo {
  pub wallet_token: WalletToken,
}

#[derive(Deserialize)]
pub struct ModeratorAddBankedBalanceToWalletRequest {
  pub credits: u16,
}

#[derive(Serialize)]
pub struct ModeratorAddBankedBalanceToWalletResponse {
  pub success: bool,
}
