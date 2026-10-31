use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for folders.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize, Default)]
pub struct FolderToken(pub String);
impl_client_token!(FolderToken);
