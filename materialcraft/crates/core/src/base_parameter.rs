use serde::{Deserialize, Serialize};

use crate::{BaseParameters, OutputFormat, ParameterInheritance, SizeMode, Value, ValueType};

/// Native base-parameter identities are distinct from node-specific parameter identifiers
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BaseParameter {
    OutputSize,
    Format,
    PixelSize,
    PixelRatio,
    Tiling,
    RandomSeed,
}

impl BaseParameter {
    pub const ALL: [Self; 6] = [Self::OutputSize, Self::Format, Self::PixelSize, Self::PixelRatio, Self::Tiling, Self::RandomSeed];

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|parameter| parameter.identifier() == name)
    }

    pub fn identifier(self) -> &'static str {
        match self {
            Self::OutputSize => "outputsize",
            Self::Format => "format",
            Self::PixelSize => "pixelsize",
            Self::PixelRatio => "pixelratio",
            Self::Tiling => "tiling",
            Self::RandomSeed => "randomseed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OutputSize => "Output size",
            Self::Format => "Output format",
            Self::PixelSize => "Pixel size",
            Self::PixelRatio => "Pixel ratio",
            Self::Tiling => "Tiling mode",
            Self::RandomSeed => "Random seed",
        }
    }

    pub fn value_type(self) -> ValueType {
        match self {
            Self::OutputSize => ValueType::Integer2,
            Self::PixelSize => ValueType::Float2,
            _ => ValueType::Integer,
        }
    }

    pub fn value(self, base: &BaseParameters) -> Value {
        match self {
            Self::OutputSize => Value::Int2(base.size),
            Self::Format => Value::Int(match base.format {
                OutputFormat::U8 => 0,
                OutputFormat::Inherit | OutputFormat::U16 => 1,
                OutputFormat::F16 => 2,
                OutputFormat::F32 => 3,
            }),
            Self::PixelSize => Value::Float2(base.pixel_size),
            Self::PixelRatio => Value::Int(base.pixel_ratio),
            Self::Tiling => Value::Int(i32::from(base.tiling[0]) | (i32::from(base.tiling[1]) << 1)),
            Self::RandomSeed => Value::Int(base.seed),
        }
    }

    pub fn set_value(self, base: &mut BaseParameters, value: &Value) -> Result<(), String> {
        if ValueType::of(value) != Some(self.value_type()) {
            return Err(format!("{} requires {}", self.label(), self.value_type().label()));
        }

        match (self, value) {
            (Self::OutputSize, Value::Int2(size)) => base.size = size.map(|component| component.clamp(-12, 13)),
            (Self::Format, Value::Int(format)) => {
                base.format = match format {
                    0 => OutputFormat::U8,
                    1 => OutputFormat::U16,
                    2 => OutputFormat::F16,
                    3 => OutputFormat::F32,
                    _ => return Err("Output format must be between 0 and 3".into()),
                };
            }
            (Self::PixelSize, Value::Float2(size)) => {
                if size.iter().any(|component| !component.is_finite() || *component <= 0.0 || *component > 8192.0) {
                    return Err("Pixel size components must be finite and between 0 and 8192, excluding zero".into());
                }

                base.pixel_size = *size;
            }
            (Self::PixelRatio, Value::Int(ratio)) if (0..=1).contains(ratio) => base.pixel_ratio = *ratio,
            (Self::Tiling, Value::Int(tiling)) if (0..=3).contains(tiling) => base.tiling = [tiling & 1 != 0, tiling & 2 != 0],
            (Self::RandomSeed, Value::Int(seed)) => base.seed = *seed,
            _ => return Err(format!("{} is outside its supported range", self.label())),
        }

        Ok(())
    }

    pub fn inheritance(self, base: &BaseParameters) -> ParameterInheritance {
        match self {
            Self::OutputSize => match base.size_mode {
                SizeMode::Absolute => ParameterInheritance::Absolute,
                SizeMode::RelativeToInput => ParameterInheritance::RelativeToInput,
                SizeMode::RelativeToParent => ParameterInheritance::RelativeToParent,
            },
            Self::Format if base.format == OutputFormat::Inherit => ParameterInheritance::RelativeToInput,
            Self::Format => base.format_mode,
            Self::PixelSize => base.pixel_size_mode,
            Self::PixelRatio => base.pixel_ratio_mode,
            Self::Tiling => base.tiling_mode,
            Self::RandomSeed => base.seed_mode,
        }
    }

    pub fn set_inheritance(self, base: &mut BaseParameters, inheritance: ParameterInheritance) {
        match self {
            Self::OutputSize => {
                base.size_mode = match inheritance {
                    ParameterInheritance::Absolute => SizeMode::Absolute,
                    ParameterInheritance::RelativeToInput => SizeMode::RelativeToInput,
                    ParameterInheritance::RelativeToParent => SizeMode::RelativeToParent,
                };
            }
            Self::Format => {
                if base.format == OutputFormat::Inherit {
                    base.format = OutputFormat::U16;
                }

                base.format_mode = inheritance;
            }
            Self::PixelSize => base.pixel_size_mode = inheritance,
            Self::PixelRatio => base.pixel_ratio_mode = inheritance,
            Self::Tiling => base.tiling_mode = inheritance,
            Self::RandomSeed => base.seed_mode = inheritance,
        }
    }
}
