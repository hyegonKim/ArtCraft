//! 3D scene staging: multi-light studio rigs, ground-plane alignment & shadow catcher,
//! and 35mm full-frame camera focal-length conversions.

use serde::{Deserialize, Serialize};

/// Soft rectangular area/directional light in a multi-light studio rig (degrees & linear HDR RGB).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StudioLight {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub half_width_deg: f32,
    pub half_height_deg: f32,
    pub color: [f32; 3],
}

/// Multi-light studio rig presets for 3D product & scene staging.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StudioRig {
    #[default]
    None,
    ProductThreePoint,
    PackshotRim,
    TabletopWarm,
    DramaticLowKey,
}

const THREE_POINT_LIGHTS: [StudioLight; 3] = [
    // Primary key softbox
    StudioLight { azimuth_deg: 38.0, elevation_deg: 34.0, half_width_deg: 20.0, half_height_deg: 16.0, color: [9.5, 9.2, 8.8] },
    // Cool fill softbox
    StudioLight { azimuth_deg: -52.0, elevation_deg: 22.0, half_width_deg: 32.0, half_height_deg: 24.0, color: [2.8, 3.1, 3.6] },
    // Crisp back rim strip
    StudioLight { azimuth_deg: 165.0, elevation_deg: 48.0, half_width_deg: 10.0, half_height_deg: 28.0, color: [6.5, 6.6, 7.0] },
];

const PACKSHOT_RIM_LIGHTS: [StudioLight; 4] = [
    // Overhead soft bank
    StudioLight { azimuth_deg: 0.0, elevation_deg: 72.0, half_width_deg: 36.0, half_height_deg: 16.0, color: [7.5, 7.5, 7.8] },
    // Left silhouette rim strip
    StudioLight { azimuth_deg: 132.0, elevation_deg: 26.0, half_width_deg: 7.0, half_height_deg: 34.0, color: [11.0, 11.2, 11.8] },
    // Right silhouette rim strip
    StudioLight { azimuth_deg: -132.0, elevation_deg: 26.0, half_width_deg: 7.0, half_height_deg: 34.0, color: [11.0, 11.2, 11.8] },
    // Subtle front fill card
    StudioLight { azimuth_deg: 8.0, elevation_deg: 16.0, half_width_deg: 28.0, half_height_deg: 20.0, color: [2.2, 2.2, 2.3] },
];

const TABLETOP_WARM_LIGHTS: [StudioLight; 3] = [
    // Warm sunlit / tungsten key
    StudioLight { azimuth_deg: 45.0, elevation_deg: 28.0, half_width_deg: 22.0, half_height_deg: 18.0, color: [11.0, 8.2, 5.4] },
    // Soft overhead bounce diffuser
    StudioLight { azimuth_deg: -20.0, elevation_deg: 68.0, half_width_deg: 38.0, half_height_deg: 24.0, color: [2.6, 2.4, 2.1] },
    // Cool ambient window fill
    StudioLight { azimuth_deg: -120.0, elevation_deg: 24.0, half_width_deg: 30.0, half_height_deg: 20.0, color: [1.8, 2.2, 3.0] },
];

const DRAMATIC_LOW_KEY_LIGHTS: [StudioLight; 2] = [
    // Narrow high-contrast grazing key
    StudioLight { azimuth_deg: 62.0, elevation_deg: 24.0, half_width_deg: 11.0, half_height_deg: 14.0, color: [16.5, 15.4, 14.0] },
    // Razor-sharp cool edge kicker
    StudioLight { azimuth_deg: -145.0, elevation_deg: 36.0, half_width_deg: 5.5, half_height_deg: 28.0, color: [5.5, 6.6, 8.8] },
];

impl StudioRig {
    pub const ALL: [StudioRig; 5] = [StudioRig::None, StudioRig::ProductThreePoint, StudioRig::PackshotRim, StudioRig::TabletopWarm, StudioRig::DramaticLowKey];

    pub fn id(self) -> &'static str {
        match self {
            StudioRig::None => "none",
            StudioRig::ProductThreePoint => "productThreePoint",
            StudioRig::PackshotRim => "packshotRim",
            StudioRig::TabletopWarm => "tabletopWarm",
            StudioRig::DramaticLowKey => "dramaticLowKey",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StudioRig::None => "Environment only",
            StudioRig::ProductThreePoint => "Product Three-Point",
            StudioRig::PackshotRim => "Packshot Rim",
            StudioRig::TabletopWarm => "Tabletop Warm",
            StudioRig::DramaticLowKey => "Dramatic Low-Key",
        }
    }

    pub fn environment_name(self) -> Option<&'static str> {
        match self {
            StudioRig::None => None,
            other => Some(other.label()),
        }
    }

    pub fn parse(s: &str) -> Option<StudioRig> {
        let norm = |x: &str| x.to_ascii_lowercase().replace([' ', '_', '-'], "");
        let target = norm(s);
        StudioRig::ALL.into_iter().find(|r| norm(r.id()) == target || norm(r.label()) == target)
    }

    pub fn lights(self) -> &'static [StudioLight] {
        match self {
            StudioRig::None => &[],
            StudioRig::ProductThreePoint => &THREE_POINT_LIGHTS,
            StudioRig::PackshotRim => &PACKSHOT_RIM_LIGHTS,
            StudioRig::TabletopWarm => &TABLETOP_WARM_LIGHTS,
            StudioRig::DramaticLowKey => &DRAMATIC_LOW_KEY_LIGHTS,
        }
    }

    /// Ambient dome `(zenith, horizon, ground)` linear RGB for procedural IBL synthesis.
    pub fn dome_colors(self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        match self {
            StudioRig::None | StudioRig::ProductThreePoint => ([0.34, 0.36, 0.40], [0.22, 0.22, 0.24], [0.06, 0.06, 0.06]),
            StudioRig::PackshotRim => ([0.20, 0.21, 0.24], [0.12, 0.12, 0.14], [0.05, 0.05, 0.06]),
            StudioRig::TabletopWarm => ([0.38, 0.34, 0.28], [0.28, 0.23, 0.18], [0.12, 0.09, 0.06]),
            StudioRig::DramaticLowKey => ([0.018, 0.020, 0.028], [0.012, 0.012, 0.016], [0.004, 0.004, 0.006]),
        }
    }

    /// Primary shadow-casting key light `(dir_x, dir_y, dir_z, intensity)` in environment space.
    pub fn key_light_vector(self, rig_intensity: f32, rig_rotation_deg: f32) -> [f32; 4] {
        let Some(primary) = self.lights().first() else {
            return [0.5, 0.8, 0.4, 0.0];
        };
        let az = (primary.azimuth_deg + rig_rotation_deg).to_radians();
        let el = primary.elevation_deg.to_radians();
        let cos_el = el.cos();
        let dx = cos_el * az.cos();
        let dy = el.sin();
        let dz = cos_el * az.sin();
        let len = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-5);
        let lum = 0.2126 * primary.color[0] + 0.7152 * primary.color[1] + 0.0722 * primary.color[2];
        let intensity = (lum * 0.28 * rig_intensity.clamp(0.0, 4.0)).clamp(0.0, 12.0);
        [dx / len, dy / len, dz / len, intensity]
    }
}

/// 3D stage scene settings: studio lighting rig, ground alignment, and ground shadow catcher.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StageSettings {
    pub rig: StudioRig,
    pub rig_intensity: f32,
    pub rig_rotation: f32,
    pub shadow_catcher: bool,
    pub ground_y: f32,
    pub ground_auto: bool,
    pub model_offset_y: f32,
    pub shadow_opacity: f32,
    pub contact_ao: f32,
    pub contact_ao_radius: f32,
    pub light_softness: f32,
}

impl Default for StageSettings {
    fn default() -> Self {
        StageSettings {
            rig: StudioRig::None,
            rig_intensity: 1.0,
            rig_rotation: 0.0,
            shadow_catcher: false,
            ground_y: 0.0,
            ground_auto: true,
            model_offset_y: 0.0,
            shadow_opacity: 0.65,
            contact_ao: 0.75,
            contact_ao_radius: 1.0,
            light_softness: 0.35,
        }
    }
}

impl StageSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Analytic ground-plane shadow catcher opacity (0..1) at world `(x, z)` combining
    /// proximity contact ambient occlusion and directional soft shadow falloff.
    pub fn sample_shadow_catcher(&self, point_xz: [f32; 2], bounds_min: [f32; 3], bounds_max: [f32; 3], key_light_dir: [f32; 3]) -> f32 {
        if !self.shadow_catcher || self.shadow_opacity <= 1e-4 {
            return 0.0;
        }
        let cx = 0.5 * (bounds_min[0] + bounds_max[0]);
        let cy = 0.5 * (bounds_min[1] + bounds_max[1]);
        let cz = 0.5 * (bounds_min[2] + bounds_max[2]);
        let rx = (0.5 * (bounds_max[0] - bounds_min[0])).max(1e-3);
        let ry = (0.5 * (bounds_max[1] - bounds_min[1])).max(1e-3);
        let rz = (0.5 * (bounds_max[2] - bounds_min[2])).max(1e-3);
        let foot_r = (rx.hypot(rz) * 0.75).max(1e-3);

        let ground_y = if self.ground_auto { bounds_min[1] } else { self.ground_y };
        let gap = (bounds_min[1] - ground_y).max(0.0);

        // 1. Analytic contact AO directly beneath the mesh footprint
        let ao_r = (foot_r * self.contact_ao_radius.clamp(0.1, 5.0)).max(1e-3);
        let dx = point_xz[0] - cx;
        let dz = point_xz[1] - cz;
        let dist_sq = (dx * dx + dz * dz) / (ao_r * ao_r);
        let gap_atten = 1.0 / (1.0 + 4.0 * (gap / ao_r) * (gap / ao_r));
        let ao_occ = (self.contact_ao.clamp(0.0, 1.0) * (-1.6 * dist_sq).exp() * gap_atten).clamp(0.0, 1.0);

        // 2. Directional soft shadow projected from the mesh volume onto y = ground_y
        let l_len = (key_light_dir[0] * key_light_dir[0] + key_light_dir[1] * key_light_dir[1] + key_light_dir[2] * key_light_dir[2]).sqrt();
        let dir_occ = if l_len > 1e-4 {
            let lx = key_light_dir[0] / l_len;
            let ly = (key_light_dir[1] / l_len).max(0.15);
            let lz = key_light_dir[2] / l_len;
            let height_above = (cy - ground_y).max(ry * 0.5);
            let proj_x = cx - lx * (height_above / ly);
            let proj_z = cz - lz * (height_above / ly);

            let penumbra = 1.0 + self.light_softness.clamp(0.0, 1.0) * (1.2 + height_above / foot_r);
            let shadow_r = foot_r * penumbra;
            let sdx = point_xz[0] - proj_x;
            let sdz = point_xz[1] - proj_z;
            let s_dist = (sdx * sdx + sdz * sdz).sqrt() / shadow_r.max(1e-3);
            let edge = 0.45 + 0.55 * self.light_softness.clamp(0.0, 1.0);
            let t = ((1.15 - s_dist) / edge.max(0.1)).clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            smooth * (0.75 / penumbra.sqrt()).clamp(0.2, 0.85)
        } else {
            0.0
        };

        let combined = 1.0 - (1.0 - ao_occ) * (1.0 - dir_occ);
        (self.shadow_opacity.clamp(0.0, 1.0) * combined).clamp(0.0, 1.0)
    }
}

/// Convert vertical field-of-view (degrees) to 35mm full-frame focal length (mm, 24mm sensor height).
pub fn fov_deg_to_focal_mm(fov_deg: f32) -> f32 {
    let fov_clamped = if fov_deg.is_finite() { fov_deg.clamp(5.0, 150.0) } else { 40.0 };
    let half_tan = (fov_clamped.to_radians() * 0.5).tan().max(1e-4);
    (12.0 / half_tan).clamp(3.0, 300.0)
}

/// Convert 35mm full-frame focal length (mm, 24mm sensor height) to vertical field-of-view (degrees).
pub fn focal_mm_to_fov_deg(focal_mm: f32) -> f32 {
    let f = if focal_mm.is_finite() { focal_mm.clamp(3.0, 300.0) } else { 33.0 };
    (2.0 * (12.0 / f).atan()).to_degrees().clamp(5.0, 150.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focal_length_and_fov_round_trip() {
        for mm in [24.0_f32, 35.0, 50.0, 85.0, 135.0] {
            let fov = focal_mm_to_fov_deg(mm);
            let back = fov_deg_to_focal_mm(fov);
            assert!((back - mm).abs() < 1e-3, "mm={mm} -> fov={fov} -> back={back}");
        }
    }

    #[test]
    fn studio_rigs_and_shadow_catcher_behave_physically() {
        for rig in StudioRig::ALL {
            assert_eq!(StudioRig::parse(rig.id()), Some(rig));
            assert_eq!(StudioRig::parse(rig.label()), Some(rig));
            if rig != StudioRig::None {
                assert!(!rig.lights().is_empty());
                assert!(rig.key_light_vector(1.0, 0.0)[3] > 0.1);
            }
        }

        let stage = StageSettings { shadow_catcher: true, ground_auto: false, ground_y: 0.0, ..StageSettings::default() };
        let lo = [-1.0, 0.0, -1.0];
        let hi = [1.0, 2.0, 1.0];
        let key = [0.7, 0.7, 0.0];
        let center_alpha = stage.sample_shadow_catcher([0.0, 0.0], lo, hi, key);
        let cast_side = stage.sample_shadow_catcher([-1.1, 0.0], lo, hi, key);
        let lit_side = stage.sample_shadow_catcher([1.1, 0.0], lo, hi, key);
        let far_alpha = stage.sample_shadow_catcher([12.0, 12.0], lo, hi, key);

        assert!(center_alpha > 0.35, "contact AO under object should be strong: {center_alpha}");
        assert!(cast_side > lit_side, "shadow receiver should be darker on cast shadow side ({cast_side}) than lit side ({lit_side})");
        assert!(far_alpha < 0.02, "shadow catcher should fade to zero far from object: {far_alpha}");
    }
}
