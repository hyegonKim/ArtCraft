use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;


/// The primary key for tasks (Tauri / Sqlite)
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl_string_token!(TaskId);
impl_crockford_generator!(TaskId, 32usize, "task_", crate::CROCKFORD_MIXED_CASE_CHARSET);
