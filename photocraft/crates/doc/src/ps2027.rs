//! Photoshop 2027 (28.0) non-destructive layer extensions:
//! - NextGen Adjustments/Filters (`Fireflies`, `WaterRipple`)
//! - 3D Relight / Lightshop Layer (`Relight3d`, up to 3 3D lights)
//! - Generative Restyle Layer (`Restyle`, preset + prompt + 0..=100% intensity)
//! - Clipboard payload for `Paste Text Attributes`.

use serde::{Deserialize, Serialize};

/// One interactive 3D point/spot lamp in a Relight (`Lightshop`) layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RelightLamp {
    pub enabled: bool,
    /// Normalized horizontal light position (`0.0..=1.0` across canvas width).
    pub x: f32,
    /// Normalized vertical light position (`0.0..=1.0` across canvas height).
    pub y: f32,
    /// Normalized elevation above the estimated depth-normal surface (`0.05..=2.0`).
    pub z: f32,
    /// Radiant intensity multiplier (`0.0..=3.0`).
    pub intensity: f32,
    /// Linear/sRGB lamp tint `[r, g, b]` in `0.0..=1.0`.
    pub color: [f32; 3],
    /// Spatial spread / falloff radius (`0.05..=2.0`).
    pub spread: f32,
}

impl Default for RelightLamp {
    fn default() -> Self {
        Self { enabled: true, x: 0.35, y: 0.30, z: 0.55, intensity: 1.15, color: [1.0, 0.92, 0.80], spread: 0.65 }
    }
}

impl RelightLamp {
    /// Default 3-light studio rig (Key warm active, Cool fill active, Rim accent standby).
    pub fn default_rig() -> [Self; 3] {
        [
            Self { enabled: true, x: 0.30, y: 0.28, z: 0.55, intensity: 1.20, color: [1.0, 0.90, 0.76], spread: 0.68 },
            Self { enabled: true, x: 0.74, y: 0.62, z: 0.65, intensity: 0.55, color: [0.72, 0.86, 1.0], spread: 0.80 },
            Self { enabled: false, x: 0.50, y: 0.14, z: 0.42, intensity: 0.85, color: [1.0, 0.58, 0.82], spread: 0.50 },
        ]
    }

    pub fn sanitized(self) -> Self {
        let finite = |v: f32, def: f32| if v.is_finite() { v } else { def };
        Self {
            enabled: self.enabled,
            x: finite(self.x, 0.5).clamp(-0.5, 1.5),
            y: finite(self.y, 0.5).clamp(-0.5, 1.5),
            z: finite(self.z, 0.55).clamp(0.05, 2.5),
            intensity: finite(self.intensity, 1.0).clamp(0.0, 4.0),
            color: [finite(self.color[0], 1.0).clamp(0.0, 1.0), finite(self.color[1], 1.0).clamp(0.0, 1.0), finite(self.color[2], 1.0).clamp(0.0, 1.0)],
            spread: finite(self.spread, 0.65).clamp(0.05, 3.0),
        }
    }
}

/// Built-in artistic transfer styles for Photoshop 2027's Restyle layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestylePreset {
    #[default]
    OilPainting,
    Watercolor,
    CyberpunkNeon,
    VintageFilm,
    PencilSketch,
    PopArt,
    AnimeCel,
    NoirInk,
}

impl RestylePreset {
    pub const ALL: [Self; 8] =
        [Self::OilPainting, Self::Watercolor, Self::CyberpunkNeon, Self::VintageFilm, Self::PencilSketch, Self::PopArt, Self::AnimeCel, Self::NoirInk];

    pub fn id(self) -> &'static str {
        match self {
            Self::OilPainting => "oilPainting",
            Self::Watercolor => "watercolor",
            Self::CyberpunkNeon => "cyberpunkNeon",
            Self::VintageFilm => "vintageFilm",
            Self::PencilSketch => "pencilSketch",
            Self::PopArt => "popArt",
            Self::AnimeCel => "animeCel",
            Self::NoirInk => "noirInk",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OilPainting => "Oil Painting",
            Self::Watercolor => "Watercolor",
            Self::CyberpunkNeon => "Cyberpunk Neon",
            Self::VintageFilm => "Vintage Film",
            Self::PencilSketch => "Pencil Sketch",
            Self::PopArt => "Pop Art",
            Self::AnimeCel => "Anime Cel",
            Self::NoirInk => "Noir Ink",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id().eq_ignore_ascii_case(s) || p.label().eq_ignore_ascii_case(s))
    }
}

/// Non-destructive backdrop-processing layer effect introduced in Photoshop 2027 (28.0).
///
/// Stored on [`crate::Layer::live_fx`] so binary PSD `Adjustment` keys stay 1-to-1 while `.pcraft`
/// preserves full parametric non-destructive editing, masking, blend modes, and clipping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LiveBackdropFx {
    /// NextGen `Fireflies` (`반딧불이`): glowing depth-varied bokeh firefly particles.
    Fireflies {
        /// Number of synthesized firefly particles (`1..=300`).
        quantity: u32,
        /// Depth spread controlling near/far size & defocus variance (`0.0..=100.0`).
        #[serde(rename = "depthVariation")]
        depth_variation: f32,
        /// Overall optical defocus / bokeh softness (`0.0..=100.0`).
        blurriness: f32,
        /// Base firefly glow tint `[r, g, b]` in `0.0..=1.0`.
        color: [f32; 3],
        /// Animation / drift phase (`0.0..=100.0`).
        progress: f32,
        /// Deterministic placement seed.
        #[serde(default = "default_seed")]
        seed: u32,
    },
    /// NextGen `Water Ripple` (`물결`): horizon-aware planar reflection + perspective wave distortion.
    WaterRipple {
        /// Normalized horizon line position (`0.05..=0.95` from top of canvas).
        horizon: f32,
        /// Wave distortion amplitude (`0.0..=100.0`).
        amplitude: f32,
        /// Spatial wave frequency (`1.0..=100.0`).
        frequency: f32,
        /// Wave travel phase (`0.0..=100.0`).
        progress: f32,
    },
    /// Non-destructive 3D Relight / Lightshop adjustment (`새 재조명 레이어…`).
    Relight3d {
        /// Retained ambient illumination (`0.0..=1.5`).
        ambient: f32,
        /// Depth-from-luminance normal strength (`0.0..=2.0`).
        #[serde(rename = "surfaceRelief")]
        surface_relief: f32,
        /// Up to 3 interactive 3D lamps.
        lamps: [RelightLamp; 3],
    },
    /// Non-destructive `Restyle` (`스타일 변경 레이어`) style-transfer adjustment.
    Restyle {
        preset: RestylePreset,
        #[serde(default)]
        prompt: String,
        /// Blend intensity in percent (`0.0..=100.0`).
        intensity: f32,
    },
}

fn default_seed() -> u32 {
    2027
}

impl LiveBackdropFx {
    pub fn default_fireflies() -> Self {
        Self::Fireflies { quantity: 48, depth_variation: 65.0, blurriness: 45.0, color: [1.0, 0.84, 0.32], progress: 25.0, seed: 2027 }
    }

    pub fn default_water_ripple() -> Self {
        Self::WaterRipple { horizon: 0.58, amplitude: 42.0, frequency: 36.0, progress: 20.0 }
    }

    pub fn default_relight_3d() -> Self {
        Self::Relight3d { ambient: 0.55, surface_relief: 1.0, lamps: RelightLamp::default_rig() }
    }

    pub fn default_restyle() -> Self {
        Self::Restyle { preset: RestylePreset::OilPainting, prompt: String::new(), intensity: 80.0 }
    }

    pub fn default_layer_name(&self) -> &'static str {
        match self {
            Self::Fireflies { .. } => "Fireflies 1",
            Self::WaterRipple { .. } => "Water Ripple 1",
            Self::Relight3d { .. } => "Relight 1",
            Self::Restyle { .. } => "Restyle 1",
        }
    }
}

/// Copied character + paragraph formatting for `Edit > Paste Text Attributes` (`텍스트 속성 붙여넣기`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextAttributeClipboard {
    pub char_style: crate::text::CharStyle,
    pub paragraph_style: crate::text::ParagraphStyle,
    pub antialias: crate::text::AntiAlias,
    pub orientation: crate::text::Orientation,
}
