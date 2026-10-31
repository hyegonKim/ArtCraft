use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for api_keys.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default)]
pub struct ApiKeyToken(pub String);
impl_client_token!(ApiKeyToken);
