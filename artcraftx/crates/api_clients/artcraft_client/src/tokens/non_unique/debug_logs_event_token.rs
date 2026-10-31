use serde::Deserialize;
use serde::Serialize;


/// A non-unique event token for the `debug_logs` table.
/// Multiple log rows can share the same event token.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct DebugLogEventToken(pub String);

impl_client_token!(DebugLogEventToken);
