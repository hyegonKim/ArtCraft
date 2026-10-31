use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// Primary key for the `user_subscriptions` table.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct UserSubscriptionToken(pub String);

impl_client_token!(UserSubscriptionToken);
