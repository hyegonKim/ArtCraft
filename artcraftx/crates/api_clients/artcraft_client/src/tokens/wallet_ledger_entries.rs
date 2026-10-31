use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for wallet_ledger_entries
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct WalletLedgerEntryToken(pub String);

impl_client_token!(WalletLedgerEntryToken);
