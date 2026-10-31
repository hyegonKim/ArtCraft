use crate::configs::stripe_artcraft_metadata_keys::STRIPE_ARTCRAFT_METADATA_WALLET_TOKEN;
use tokens::tokens::wallets::WalletToken;

pub (crate) fn get_metadata_wallet_token(
  metadata: &std::collections::HashMap<String, String>,
) -> Option<WalletToken> {
  metadata.get(STRIPE_ARTCRAFT_METADATA_WALLET_TOKEN)
      .map(|t| WalletToken::new_from_str(&t))
}
