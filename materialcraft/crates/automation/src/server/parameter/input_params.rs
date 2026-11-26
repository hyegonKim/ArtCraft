use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct InputParams {
    /// Input events in window points, applied in order on the next frames
    #[schemars(schema_with = "events_schema")]
    pub events: Vec<Value>,
}

/// Every event is a JSON object. Saying so keeps `"items": true` (what `Vec<Value>` produces) and
/// `"additionalProperties": true` out of `tools/list`: Claude Desktop rejects boolean schemas and
/// drops the whole server.
fn events_schema(_: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "array", "items": { "type": "object" } })
}
