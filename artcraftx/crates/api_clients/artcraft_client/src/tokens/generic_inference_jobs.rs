use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for "generic" inference jobs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub struct InferenceJobToken(String);

impl_client_token!(InferenceJobToken);
