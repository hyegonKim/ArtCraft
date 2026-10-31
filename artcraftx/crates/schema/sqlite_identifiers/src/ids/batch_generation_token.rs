use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for Audit Logs.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct BatchGenerationToken(pub String);

impl_crockford_generator!(BatchGenerationToken, 32usize, "batch_g_", crate::CROCKFORD_LOWERCASE_CHARSET);
impl_string_token!(BatchGenerationToken);
