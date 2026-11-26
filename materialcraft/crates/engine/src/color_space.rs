use serde::{Deserialize, Serialize};

/// How an exported output's values are written
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorSpace {
    /// Color written through the sRGB curve
    Srgb,
    /// Color written as linear values
    Linear,
    /// Data written unchanged (normal, roughness, height…)
    #[default]
    Raw,
}

impl ColorSpace {
    pub const ALL: [ColorSpace; 3] = [ColorSpace::Srgb, ColorSpace::Linear, ColorSpace::Raw];

    /// The id `file.export` takes in `colorSpaces`
    pub fn id(self) -> &'static str {
        match self {
            ColorSpace::Srgb => "srgb",
            ColorSpace::Linear => "linear",
            ColorSpace::Raw => "raw",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ColorSpace::Srgb => "sRGB",
            ColorSpace::Linear => "Linear",
            ColorSpace::Raw => "Raw",
        }
    }

    pub fn from_id(id: &str) -> Option<ColorSpace> {
        ColorSpace::ALL.into_iter().find(|space| space.id().eq_ignore_ascii_case(id))
    }

    /// sRGB for the usages that carry color (base color, emissive…), raw for data maps
    pub fn for_usage(usage: &str) -> ColorSpace {
        if materialcraft_formats::material::srgb_usage(usage) { ColorSpace::Srgb } else { ColorSpace::Raw }
    }

    pub fn encodes_srgb(self) -> bool {
        self == ColorSpace::Srgb
    }
}
