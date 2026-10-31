use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for users.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub struct AppSessionToken(pub String);
impl_client_token!(AppSessionToken);
