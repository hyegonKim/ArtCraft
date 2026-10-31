use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for the  "model_weights" table.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ModelWeightToken(pub String);

impl_client_token!(ModelWeightToken);
