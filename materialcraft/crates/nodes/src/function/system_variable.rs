use materialcraft_core::{Value, ValueType};

use super::{EvaluationContext, FunctionScope};

/// SBS variable identifiers are part of the interchange contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemVariable {
    NormalFormat,
    Tiling,
    PixelRatio,
    UvTile,
    PhysicalSize,
    Size,
    SizeLog2,
    Time,
    Position,
    Depth,
    DepthPow2,
    Number,
}

impl SystemVariable {
    pub const ALL: [Self; 12] = [
        Self::NormalFormat,
        Self::Tiling,
        Self::PixelRatio,
        Self::UvTile,
        Self::PhysicalSize,
        Self::Size,
        Self::SizeLog2,
        Self::Time,
        Self::Position,
        Self::Depth,
        Self::DepthPow2,
        Self::Number,
    ];

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|variable| variable.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::NormalFormat => "$normalformat",
            Self::Tiling => "$tiling",
            Self::PixelRatio => "$pixelratio",
            Self::UvTile => "$uvtile",
            Self::PhysicalSize => "$physicalsize",
            Self::Size => "$size",
            Self::SizeLog2 => "$sizelog2",
            Self::Time => "$time",
            Self::Position => "$pos",
            Self::Depth => "$depth",
            Self::DepthPow2 => "$depthpow2",
            Self::Number => "$number",
        }
    }

    pub fn value_type(self) -> ValueType {
        match self {
            Self::NormalFormat | Self::Tiling | Self::PixelRatio => ValueType::Integer,
            Self::UvTile => ValueType::Integer2,
            Self::PhysicalSize => ValueType::Float3,
            Self::Size | Self::SizeLog2 | Self::Position => ValueType::Float2,
            Self::Time | Self::Depth | Self::DepthPow2 | Self::Number => ValueType::Float,
        }
    }

    pub fn available_in(self, scope: FunctionScope) -> bool {
        match self {
            Self::Position => matches!(scope, FunctionScope::PixelProcessor | FunctionScope::FxMap),
            Self::Depth | Self::DepthPow2 | Self::Number => scope == FunctionScope::FxMap,
            _ => true,
        }
    }

    pub(super) fn resolve(self, context: &EvaluationContext<'_>) -> Result<Option<Value>, String> {
        if !self.available_in(context.scope) {
            return Ok(None);
        }

        let value = match self {
            Self::Position => Value::Float2(context.position),
            Self::Size => Value::Float2(context.size),
            Self::SizeLog2 => Value::Float2(context.size.map(f32::log2)),
            _ => context.variables.get(self.name()).cloned().unwrap_or_else(|| match self {
                Self::Tiling => Value::Int(if context.tile { 3 } else { 0 }),
                Self::DepthPow2 => Value::Float(1.0),
                _ => self.value_type().default_value(),
            }),
        };

        if ValueType::of(&value) != Some(self.value_type()) {
            return Err(format!("system variable `{}` has the wrong type; expected {}", self.name(), self.value_type().label()));
        }

        if matches!(self.value_type(), ValueType::Float | ValueType::Float2 | ValueType::Float3)
            && super::value_type::floats(&value)?.iter().any(|component| !component.is_finite())
        {
            return Err(format!("system variable `{}` must be finite", self.name()));
        }

        match (&self, &value) {
            (Self::Tiling, Value::Int(value)) if !(0..=3).contains(value) => return Err("$tiling must be between 0 and 3".into()),
            (Self::PixelRatio, Value::Int(value)) if !(0..=1).contains(value) => return Err("$pixelratio must be 0 or 1".into()),
            _ => {}
        }

        Ok(Some(value))
    }
}
