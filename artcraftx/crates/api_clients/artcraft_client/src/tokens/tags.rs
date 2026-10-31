use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for Prompts
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TagToken(pub String);

impl_client_token!(TagToken);
