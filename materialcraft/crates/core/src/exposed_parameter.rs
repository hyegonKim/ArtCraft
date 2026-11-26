use serde::{Deserialize, Serialize};

use crate::{ParameterEditor, Value};

/// The kind of value an exposed graph input carries
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedParameter {
    /// Identifier, unique in the graph
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    pub default: Value,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<ParameterEditor>,
    #[serde(default)]
    pub clamp: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f32>,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
    /// Labels when the value is an enum
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

fn one() -> f32 {
    1.0
}

impl ExposedParameter {
    pub fn editor(&self) -> ParameterEditor {
        self.editor.filter(|editor| ParameterEditor::choices(&self.default).contains(editor)).unwrap_or_else(|| ParameterEditor::for_value(&self.default))
    }

    pub fn step(&self) -> f32 {
        self.step.filter(|step| step.is_finite() && *step > 0.0).unwrap_or(match self.default {
            Value::Int(_) | Value::Int2(_) | Value::Int3(_) | Value::Int4(_) | Value::Enum(_) => 1.0,
            _ => 0.01,
        })
    }
}
