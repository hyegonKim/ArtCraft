use crate::Value;
use serde::{Deserialize, Serialize};

/// Presentation of an exposed value in Properties
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterEditor {
    Slider,
    Input,
    Color,
    Dropdown,
    Toggle,
    Text,
}

impl ParameterEditor {
    pub fn choices(value: &Value) -> &'static [Self] {
        match value {
            Value::Bool(_) => &[Self::Toggle],
            Value::Text(_) => &[Self::Text],
            Value::Enum(_) => &[Self::Dropdown],
            Value::Color(_) => &[Self::Color, Self::Input],
            Value::Float3(_) | Value::Float4(_) => &[Self::Slider, Self::Input, Self::Color],
            Value::Float(_) | Value::Float2(_) | Value::Int(_) | Value::Int2(_) | Value::Int3(_) | Value::Int4(_) => &[Self::Slider, Self::Input],
            _ => &[],
        }
    }

    pub fn for_value(value: &Value) -> Self {
        Self::choices(value).first().copied().unwrap_or(Self::Input)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Slider => "Slider",
            Self::Input => "Input field",
            Self::Color => "Color",
            Self::Dropdown => "Drop down list",
            Self::Toggle => "Toggle",
            Self::Text => "Text",
        }
    }
}
