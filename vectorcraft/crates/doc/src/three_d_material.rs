//! Substance-style procedural PBR material presets and raytracing settings stored on 3D effects.
//!
//! Stored inside [`crate::Effect::params`] with `#[serde(default)]` so existing `.vectorcraft`,
//! `.ai`, `.svg`, and `.pdf` documents deserialize with identical appearance when these keys are
//! absent.

use serde::{Deserialize, Serialize};

use crate::Effect;

/// Procedural PBR material preset for 3D and Materials effects (`Extrude & Bevel`, `Revolve`,
/// `Inflate`, `Rotate`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubstanceMaterialPreset {
    #[default]
    None,
    BrushedAluminum,
    PolishedGold,
    SatinCopper,
    WeatheredPatina,
    FrostedAcrylic,
    CarraraMarble,
    CarbonWeave,
    GlazedCeramic,
    MatteClay,
}

impl SubstanceMaterialPreset {
    pub const ALL: [Self; 10] = [
        Self::None,
        Self::BrushedAluminum,
        Self::PolishedGold,
        Self::SatinCopper,
        Self::WeatheredPatina,
        Self::FrostedAcrylic,
        Self::CarraraMarble,
        Self::CarbonWeave,
        Self::GlazedCeramic,
        Self::MatteClay,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BrushedAluminum => "brushedAluminum",
            Self::PolishedGold => "polishedGold",
            Self::SatinCopper => "satinCopper",
            Self::WeatheredPatina => "weatheredPatina",
            Self::FrostedAcrylic => "frostedAcrylic",
            Self::CarraraMarble => "carraraMarble",
            Self::CarbonWeave => "carbonWeave",
            Self::GlazedCeramic => "glazedCeramic",
            Self::MatteClay => "matteClay",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::BrushedAluminum => "Brushed Aluminum",
            Self::PolishedGold => "Polished Gold",
            Self::SatinCopper => "Satin Copper",
            Self::WeatheredPatina => "Weathered Patina",
            Self::FrostedAcrylic => "Frosted Acrylic",
            Self::CarraraMarble => "Carrara Marble",
            Self::CarbonWeave => "Carbon Weave",
            Self::GlazedCeramic => "Glazed Ceramic",
            Self::MatteClay => "Matte Clay",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == s)
    }
}

fn default_roughness() -> f64 {
    50.0
}

fn default_scale() -> f64 {
    100.0
}

fn default_normal_strength() -> f64 {
    100.0
}

fn default_ao() -> f64 {
    50.0
}

/// Serializable PBR material and physical raytracing parameters on a 3D [`Effect`].
///
/// Percentages match the 3D effect parameter catalog (`roughness`: `0..=100`, `metallic`: `0..=100`,
/// `materialScale`: `10..=500`, `materialWear`: `0..=100`, `normalStrength`: `0..=200`,
/// `ambientOcclusion`: `0..=100`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThreeDMaterialSettings {
    pub substance_preset: SubstanceMaterialPreset,
    #[serde(default = "default_roughness")]
    pub roughness: f64,
    pub metallic: f64,
    #[serde(default = "default_scale")]
    pub material_scale: f64,
    pub material_wear: f64,
    #[serde(default = "default_normal_strength")]
    pub normal_strength: f64,
    pub material_seed: u32,
    pub raytrace: bool,
    #[serde(default = "default_ao")]
    pub ambient_occlusion: f64,
}

impl Default for ThreeDMaterialSettings {
    fn default() -> Self {
        Self {
            substance_preset: SubstanceMaterialPreset::None,
            roughness: default_roughness(),
            metallic: 0.0,
            material_scale: default_scale(),
            material_wear: 0.0,
            normal_strength: default_normal_strength(),
            material_seed: 0,
            raytrace: false,
            ambient_occlusion: default_ao(),
        }
    }
}

impl ThreeDMaterialSettings {
    /// Sanitize loaded values into valid finite ranges.
    pub fn sanitized(self) -> Self {
        let finite = |v: f64, d: f64, lo: f64, hi: f64| {
            if v.is_finite() { v.clamp(lo, hi) } else { d }
        };
        Self {
            substance_preset: self.substance_preset,
            roughness: finite(self.roughness, 50.0, 0.0, 100.0),
            metallic: finite(self.metallic, 0.0, 0.0, 100.0),
            material_scale: finite(self.material_scale, 100.0, 10.0, 500.0),
            material_wear: finite(self.material_wear, 0.0, 0.0, 100.0),
            normal_strength: finite(self.normal_strength, 100.0, 0.0, 200.0),
            material_seed: self.material_seed.min(10_000),
            raytrace: self.raytrace,
            ambient_occlusion: finite(self.ambient_occlusion, 50.0, 0.0, 100.0),
        }
    }

    /// Read material and raytracing settings from a 3D effect parameter JSON value, falling back
    /// cleanly to defaults when fields are absent or malformed.
    pub fn from_params(params: &serde_json::Value) -> Self {
        let mut out = Self::default();
        let Some(obj) = params.as_object() else { return out };
        if let Some(s) = obj.get("substancePreset").and_then(serde_json::Value::as_str)
            && let Some(p) = SubstanceMaterialPreset::parse(s)
        {
            out.substance_preset = p;
        }
        let read_f64 = |key: &str, d: f64| obj.get(key).and_then(serde_json::Value::as_f64).filter(|v| v.is_finite()).unwrap_or(d);
        out.roughness = read_f64("roughness", out.roughness);
        out.metallic = read_f64("metallic", out.metallic);
        out.material_scale = read_f64("materialScale", out.material_scale);
        out.material_wear = read_f64("materialWear", out.material_wear);
        out.normal_strength = read_f64("normalStrength", out.normal_strength);
        if let Some(seed) = obj.get("materialSeed").and_then(serde_json::Value::as_u64) {
            out.material_seed = u32::try_from(seed.min(10_000)).unwrap_or(0);
        }
        if let Some(rt) = obj.get("raytrace").and_then(serde_json::Value::as_bool) {
            out.raytrace = rt;
        }
        out.ambient_occlusion = read_f64("ambientOcclusion", out.ambient_occlusion);
        out.sanitized()
    }

    /// Write these settings into a 3D effect's JSON parameter map without disturbing other keys.
    pub fn write_into(self, params: &mut serde_json::Value) {
        let s = self.sanitized();
        if !params.is_object() {
            *params = serde_json::Value::Object(serde_json::Map::new());
        }
        if let Some(map) = params.as_object_mut() {
            map.insert("substancePreset".into(), serde_json::json!(s.substance_preset.id()));
            map.insert("roughness".into(), serde_json::json!(s.roughness));
            map.insert("metallic".into(), serde_json::json!(s.metallic));
            map.insert("materialScale".into(), serde_json::json!(s.material_scale));
            map.insert("materialWear".into(), serde_json::json!(s.material_wear));
            map.insert("normalStrength".into(), serde_json::json!(s.normal_strength));
            map.insert("materialSeed".into(), serde_json::json!(s.material_seed));
            map.insert("raytrace".into(), serde_json::json!(s.raytrace));
            map.insert("ambientOcclusion".into(), serde_json::json!(s.ambient_occlusion));
        }
    }
}

impl Effect {
    /// Extract [`ThreeDMaterialSettings`] from this effect's parameters (defaults when absent).
    pub fn three_d_material(&self) -> ThreeDMaterialSettings {
        ThreeDMaterialSettings::from_params(&self.params)
    }

    /// Update this effect's parameters with `settings`.
    pub fn set_three_d_material(&mut self, settings: ThreeDMaterialSettings) {
        settings.write_into(&mut self.params);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_empty_params_deserialize_identically_to_defaults() {
        let from_empty: ThreeDMaterialSettings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(from_empty, ThreeDMaterialSettings::default());
        assert_eq!(ThreeDMaterialSettings::from_params(&json!({})), ThreeDMaterialSettings::default());
    }

    #[test]
    fn all_presets_round_trip_through_serde_and_effect_params() {
        for preset in SubstanceMaterialPreset::ALL {
            assert_eq!(SubstanceMaterialPreset::parse(preset.id()), Some(preset));
            let settings = ThreeDMaterialSettings {
                substance_preset: preset,
                roughness: 32.0,
                metallic: 75.0,
                material_scale: 140.0,
                material_wear: 25.0,
                normal_strength: 115.0,
                material_seed: 42,
                raytrace: true,
                ambient_occlusion: 65.0,
            };
            let serialized = serde_json::to_value(settings).unwrap();
            let restored: ThreeDMaterialSettings = serde_json::from_value(serialized).unwrap();
            assert_eq!(restored, settings);

            let mut fx = Effect { id: "threeD.extrude".into(), params: json!({"depth": 40.0}), visible: true };
            fx.set_three_d_material(settings);
            assert_eq!(fx.three_d_material(), settings);
            assert_eq!(fx.params["depth"], json!(40.0));
        }
    }
}
