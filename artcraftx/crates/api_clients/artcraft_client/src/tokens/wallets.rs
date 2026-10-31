use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for wallets
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct WalletToken(pub String);

impl_client_token!(WalletToken);
