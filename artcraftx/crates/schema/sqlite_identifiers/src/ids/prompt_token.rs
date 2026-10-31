use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for Prompts
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PromptToken(pub String);

impl_crockford_generator!(PromptToken, 32usize, "prompt_", crate::CROCKFORD_LOWERCASE_CHARSET);
impl_string_token!(PromptToken);
