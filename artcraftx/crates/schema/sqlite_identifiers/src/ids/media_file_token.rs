use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for Media Files
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MediaFileToken(pub String);

impl_string_token!(MediaFileToken);
impl_crockford_generator!(MediaFileToken, 32usize, "m_", crate::CROCKFORD_LOWERCASE_CHARSET);
