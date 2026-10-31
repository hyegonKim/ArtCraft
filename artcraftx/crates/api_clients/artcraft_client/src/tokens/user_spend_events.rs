use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for user_spend_events
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UserSpendEventToken(pub String);

impl_client_token!(UserSpendEventToken);
