use serde::{Deserialize, Serialize};

/// Host values shared by graph instances for one deterministic evaluation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EvaluationEnvironment {
    pub time_seconds: f32,
    pub uv_tile: [i32; 2],
    /// Raw host convention code; the static authoring environment uses zero.
    pub normal_format: i32,
}
