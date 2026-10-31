use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for users.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub struct UserToken(pub String);
impl_client_token!(UserToken);
// NB: Older user tokens were under this regime: 15 characters, "U:" prefix, Crockford Upper.
