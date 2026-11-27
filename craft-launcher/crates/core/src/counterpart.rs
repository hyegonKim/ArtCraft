//! Industry counterpart metadata, Creative Cloud Desktop (`KCCC v6.9.1`) FFC catalog definitions,
//! domain categories, and cross-app file routing for the Craft App Suite.
//!
//! Kept separate from the Ed25519-signed `catalog.toml` schema so the hosted startup snapshot
//! verification remains untouched while `core`, `native` (`detect.rs`), and `ui-egui` share a
//! single source of truth for counterpart display metadata, search terms, expected versions,
//! platform install paths, domain categories, and file extension routing.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::version::{self, Version};

/// All 16 official Craft Suite app identifiers represented in counterpart mapping.
pub const CRAFT_SUITE_16_IDS: [&str; 16] = [
    "photocraft",
    "vectorcraft",
    "filmcraft",
    "lightcraft",
    "pdfcraft",
    "effectcraft",
    "anicraft",
    "designcraft",
    "figcraft",
    "soundcraft",
    "wordcraft",
    "deckcraft",
    "cadcraft",
    "gridcraft",
    "texturecraft",
    "materialcraft",
];

/// Vendor of a Craft Suite app's industry counterpart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterpartVendor {
    Adobe,
    Microsoft,
    Autodesk,
    Figma,
}

/// Identifier for an Adobe Creative Cloud Desktop (`KCCC v6.9.1`) FFC product or shared component
/// tracked by counterpart detection and catalog metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AdobeProductId {
    Photoshop2027,
    CameraRaw,
    Illustrator2027,
    PremierePro2027,
    MediaEncoder2027,
    AfterEffects2027,
    InDesign2027,
    AcrobatDc,
    LightroomClassic,
    Animate,
    Audition,
    Substance3DPainter,
    Substance3DDesigner,
    CreativeCloudDesktop,
    AdobeFirefly,
    AdobeExpress,
    LightroomCc,
    AdobeBridge,
    InCopy2027,
    CharacterAnimator,
    Substance3DStager,
    Substance3DSampler,
    Substance3DModeler,
    AdobeDimension,
    AdobeFresco,
    AdobeCapture,
    AdobeXd,
    Dreamweaver,
    AcrobatDistiller,
    FrameIo,
    UxpDeveloperTools,
    CreativeCloudLibraries,
    AdobeFonts,
}

/// Static specification for an Adobe product or plug-in used by `native::detect` and UI metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdobeProductSpec {
    pub id: AdobeProductId,
    /// Canonical display name (for example `"Adobe Photoshop 2027"` or `"Adobe Acrobat DC"`).
    pub name: &'static str,
    /// Product family without year suffix (`"Adobe Photoshop"`).
    pub family: &'static str,
    /// Target Creative Cloud 2027 / DC release version string (`"28.0"`, `"26.2"`, `"19.0"`).
    pub release_version: &'static str,
    /// Minimum `(major, minor)` version tuple that satisfies the CC 2027 / DC 26.2 generation.
    pub min_cc_2027_version: (u64, u64),
    /// Whether this entry is part of the primary 8-product Adobe CC 2027 / Acrobat DC 26.2 lineup.
    pub cc_2027_lineup: bool,
    /// Optional companion component (`("Camera Raw", "19.0")` for Photoshop 2027).
    pub companion: Option<(&'static str, &'static str)>,
    /// Whether Windows paths live under Common Files (`CommonProgramW6432` / `CommonProgramFiles`)
    /// rather than `Program Files`.
    pub uses_common_files: bool,
    /// Relative Windows install directory (checked under both `<Root>\Adobe\<dir>` and `<Root>\<dir>`).
    pub windows_dir: &'static str,
    /// Relative Windows executable or plug-in binary path (checked under `<Root>\Adobe\<exe>` and `<Root>\<exe>`).
    pub windows_exe: &'static str,
    /// Older Windows executable relative paths so pre-2027 installs are also recognized with their
    /// actual file/registry version.
    pub legacy_windows_exes: &'static [&'static str],
    /// Older Windows install directories.
    pub legacy_windows_dirs: &'static [&'static str],
    /// macOS bundle relative paths under `/Applications` (or absolute `/Library/...` paths).
    pub mac_bundles: &'static [&'static str],
    /// Windows Uninstall registry `DisplayName` prefixes matched case-insensitively.
    pub registry_prefixes: &'static [&'static str],
}

impl AdobeProductSpec {
    /// Look up the specification for `id`.
    pub fn by_id(id: AdobeProductId) -> &'static AdobeProductSpec {
        // Every variant of `AdobeProductId` is present in `ADOBE_PRODUCTS`.
        ADOBE_PRODUCTS.iter().find(|p| p.id == id).unwrap_or(&ADOBE_PRODUCTS[0])
    }

    /// Formatted display metadata combining product name, release version, and any companion
    /// component (`"Adobe Photoshop 2027 28.0 (Camera Raw 19.0)"`).
    pub fn display_metadata(&self) -> String {
        match self.companion {
            Some((comp_name, comp_ver)) => format!("{} {} ({comp_name} {comp_ver})", self.name, self.release_version),
            None => format!("{} {}", self.name, self.release_version),
        }
    }

    /// Whether a parsed [`Version`] meets or exceeds this product's CC 2027 / DC 26.2 baseline.
    pub fn matches_cc_2027_version(&self, v: &Version) -> bool {
        (v.major, v.minor) >= self.min_cc_2027_version
    }

    /// Parse this product's canonical `release_version` as a [`Version`].
    pub fn parsed_release_version(&self) -> Option<Version> {
        version::parse(self.release_version)
    }
}

/// Complete Creative Cloud Desktop (`KCCC v6.9.1`) FFC catalog products and shared components
/// recognized by counterpart detection.
pub const ADOBE_PRODUCTS: [AdobeProductSpec; 33] = [
    AdobeProductSpec {
        id: AdobeProductId::Photoshop2027,
        name: "Adobe Photoshop 2027",
        family: "Adobe Photoshop",
        release_version: "28.0",
        min_cc_2027_version: (28, 0),
        cc_2027_lineup: true,
        companion: Some(("Camera Raw", "19.0")),
        uses_common_files: false,
        windows_dir: r"Adobe Photoshop 2027",
        windows_exe: r"Adobe Photoshop 2027\Photoshop.exe",
        legacy_windows_exes: &[r"Adobe Photoshop 2026\Photoshop.exe", r"Adobe Photoshop 2025\Photoshop.exe", r"Adobe Photoshop 2024\Photoshop.exe"],
        legacy_windows_dirs: &[r"Adobe Photoshop 2026", r"Adobe Photoshop 2025", r"Adobe Photoshop 2024"],
        mac_bundles: &[
            "Adobe Photoshop 2027/Adobe Photoshop 2027.app",
            "Adobe Photoshop 2026/Adobe Photoshop 2026.app",
            "Adobe Photoshop 2025/Adobe Photoshop 2025.app",
        ],
        registry_prefixes: &["Adobe Photoshop 2027", "Adobe Photoshop"],
    },
    AdobeProductSpec {
        id: AdobeProductId::CameraRaw,
        name: "Adobe Camera Raw",
        family: "Camera Raw",
        release_version: "19.0",
        min_cc_2027_version: (19, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: true,
        windows_dir: r"Plug-Ins\CC\File Formats",
        windows_exe: r"Plug-Ins\CC\File Formats\Camera Raw.8bi",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["/Library/Application Support/Adobe/Plug-Ins/CC/File Formats/Camera Raw.plugin"],
        registry_prefixes: &["Adobe Camera Raw", "Camera Raw"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Illustrator2027,
        name: "Adobe Illustrator 2027",
        family: "Adobe Illustrator",
        release_version: "31.0",
        min_cc_2027_version: (31, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Illustrator 2027",
        windows_exe: r"Adobe Illustrator 2027\Support Files\Contents\Windows\Illustrator.exe",
        legacy_windows_exes: &[
            r"Adobe Illustrator 2026\Support Files\Contents\Windows\Illustrator.exe",
            r"Adobe Illustrator 2025\Support Files\Contents\Windows\Illustrator.exe",
            r"Adobe Illustrator 2024\Support Files\Contents\Windows\Illustrator.exe",
        ],
        legacy_windows_dirs: &[r"Adobe Illustrator 2026", r"Adobe Illustrator 2025", r"Adobe Illustrator 2024"],
        mac_bundles: &[
            "Adobe Illustrator 2027/Adobe Illustrator.app",
            "Adobe Illustrator 2026/Adobe Illustrator.app",
            "Adobe Illustrator 2025/Adobe Illustrator.app",
        ],
        registry_prefixes: &["Adobe Illustrator 2027", "Adobe Illustrator"],
    },
    AdobeProductSpec {
        id: AdobeProductId::PremierePro2027,
        name: "Adobe Premiere Pro 2027",
        family: "Adobe Premiere Pro",
        release_version: "27.0",
        min_cc_2027_version: (27, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Premiere Pro 2027",
        windows_exe: r"Adobe Premiere Pro 2027\Adobe Premiere Pro.exe",
        legacy_windows_exes: &[
            r"Adobe Premiere Pro 2026\Adobe Premiere Pro.exe",
            r"Adobe Premiere Pro 2025\Adobe Premiere Pro.exe",
            r"Adobe Premiere Pro 2024\Adobe Premiere Pro.exe",
        ],
        legacy_windows_dirs: &[r"Adobe Premiere Pro 2026", r"Adobe Premiere Pro 2025", r"Adobe Premiere Pro 2024"],
        mac_bundles: &[
            "Adobe Premiere Pro 2027/Adobe Premiere Pro 2027.app",
            "Adobe Premiere Pro 2026/Adobe Premiere Pro 2026.app",
            "Adobe Premiere Pro 2025/Adobe Premiere Pro 2025.app",
        ],
        registry_prefixes: &["Adobe Premiere Pro 2027", "Adobe Premiere Pro"],
    },
    AdobeProductSpec {
        id: AdobeProductId::MediaEncoder2027,
        name: "Adobe Media Encoder 2027",
        family: "Adobe Media Encoder",
        release_version: "27.0",
        min_cc_2027_version: (27, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Media Encoder 2027",
        windows_exe: r"Adobe Media Encoder 2027\Adobe Media Encoder.exe",
        legacy_windows_exes: &[
            r"Adobe Media Encoder 2026\Adobe Media Encoder.exe",
            r"Adobe Media Encoder 2025\Adobe Media Encoder.exe",
            r"Adobe Media Encoder 2024\Adobe Media Encoder.exe",
        ],
        legacy_windows_dirs: &[r"Adobe Media Encoder 2026", r"Adobe Media Encoder 2025", r"Adobe Media Encoder 2024"],
        mac_bundles: &[
            "Adobe Media Encoder 2027/Adobe Media Encoder 2027.app",
            "Adobe Media Encoder 2026/Adobe Media Encoder 2026.app",
            "Adobe Media Encoder 2025/Adobe Media Encoder 2025.app",
        ],
        registry_prefixes: &["Adobe Media Encoder 2027", "Adobe Media Encoder"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AfterEffects2027,
        name: "Adobe After Effects 2027",
        family: "Adobe After Effects",
        release_version: "27.0",
        min_cc_2027_version: (27, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe After Effects 2027",
        windows_exe: r"Adobe After Effects 2027\Support Files\AfterFX.exe",
        legacy_windows_exes: &[
            r"Adobe After Effects 2026\Support Files\AfterFX.exe",
            r"Adobe After Effects 2025\Support Files\AfterFX.exe",
            r"Adobe After Effects 2024\Support Files\AfterFX.exe",
        ],
        legacy_windows_dirs: &[r"Adobe After Effects 2026", r"Adobe After Effects 2025", r"Adobe After Effects 2024"],
        mac_bundles: &[
            "Adobe After Effects 2027/Adobe After Effects 2027.app",
            "Adobe After Effects 2026/Adobe After Effects 2026.app",
            "Adobe After Effects 2025/Adobe After Effects 2025.app",
        ],
        registry_prefixes: &["Adobe After Effects 2027", "Adobe After Effects"],
    },
    AdobeProductSpec {
        id: AdobeProductId::InDesign2027,
        name: "Adobe InDesign 2027",
        family: "Adobe InDesign",
        release_version: "22.0",
        min_cc_2027_version: (22, 0),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe InDesign 2027",
        windows_exe: r"Adobe InDesign 2027\InDesign.exe",
        legacy_windows_exes: &[r"Adobe InDesign 2026\InDesign.exe", r"Adobe InDesign 2025\InDesign.exe", r"Adobe InDesign 2024\InDesign.exe"],
        legacy_windows_dirs: &[r"Adobe InDesign 2026", r"Adobe InDesign 2025", r"Adobe InDesign 2024"],
        mac_bundles: &[
            "Adobe InDesign 2027/Adobe InDesign 2027.app",
            "Adobe InDesign 2026/Adobe InDesign 2026.app",
            "Adobe InDesign 2025/Adobe InDesign 2025.app",
        ],
        registry_prefixes: &["Adobe InDesign 2027", "Adobe InDesign"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AcrobatDc,
        name: "Adobe Acrobat DC",
        family: "Adobe Acrobat",
        release_version: "26.2",
        min_cc_2027_version: (26, 2),
        cc_2027_lineup: true,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Acrobat DC",
        windows_exe: r"Acrobat DC\Acrobat\Acrobat.exe",
        legacy_windows_exes: &[r"Acrobat 2020\Acrobat\Acrobat.exe", r"Acrobat DC\Reader\AcroRd32.exe"],
        legacy_windows_dirs: &[r"Acrobat 2020"],
        mac_bundles: &["Adobe Acrobat DC/Adobe Acrobat.app"],
        registry_prefixes: &["Adobe Acrobat DC", "Adobe Acrobat (64-bit)", "Adobe Acrobat"],
    },
    AdobeProductSpec {
        id: AdobeProductId::LightroomClassic,
        name: "Adobe Lightroom Classic",
        family: "Adobe Lightroom Classic",
        release_version: "16.0",
        min_cc_2027_version: (16, 0),
        cc_2027_lineup: false,
        companion: Some(("Camera Raw", "19.0")),
        uses_common_files: false,
        windows_dir: r"Adobe Lightroom Classic",
        windows_exe: r"Adobe Lightroom Classic\Lightroom.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Lightroom Classic/Adobe Lightroom Classic.app"],
        registry_prefixes: &["Adobe Lightroom Classic"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Animate,
        name: "Adobe Animate",
        family: "Adobe Animate",
        release_version: "25.0",
        min_cc_2027_version: (25, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Animate 2027",
        windows_exe: r"Adobe Animate 2027\Animate.exe",
        legacy_windows_exes: &[r"Adobe Animate 2024\Animate.exe", r"Adobe Animate 2023\Animate.exe"],
        legacy_windows_dirs: &[r"Adobe Animate 2024", r"Adobe Animate 2023"],
        mac_bundles: &["Adobe Animate 2027/Adobe Animate 2027.app", "Adobe Animate 2024/Adobe Animate 2024.app"],
        registry_prefixes: &["Adobe Animate 2027", "Adobe Animate"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Audition,
        name: "Adobe Audition",
        family: "Adobe Audition",
        release_version: "27.0",
        min_cc_2027_version: (27, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Audition 2027",
        windows_exe: r"Adobe Audition 2027\Adobe Audition.exe",
        legacy_windows_exes: &[r"Adobe Audition 2026\Adobe Audition.exe", r"Adobe Audition 2025\Adobe Audition.exe"],
        legacy_windows_dirs: &[r"Adobe Audition 2026", r"Adobe Audition 2025"],
        mac_bundles: &["Adobe Audition 2027/Adobe Audition 2027.app", "Adobe Audition 2026/Adobe Audition 2026.app"],
        registry_prefixes: &["Adobe Audition 2027", "Adobe Audition"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Substance3DPainter,
        name: "Adobe Substance 3D Painter",
        family: "Adobe Substance 3D Painter",
        release_version: "11.0",
        min_cc_2027_version: (11, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Substance 3D Painter",
        windows_exe: r"Adobe Substance 3D Painter\Adobe Substance 3D Painter.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Substance 3D Painter/Adobe Substance 3D Painter.app"],
        registry_prefixes: &["Adobe Substance 3D Painter"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Substance3DDesigner,
        name: "Adobe Substance 3D Designer",
        family: "Adobe Substance 3D Designer",
        release_version: "15.0",
        min_cc_2027_version: (15, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Substance 3D Designer",
        windows_exe: r"Adobe Substance 3D Designer\Adobe Substance 3D Designer.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Substance 3D Designer/Adobe Substance 3D Designer.app"],
        registry_prefixes: &["Adobe Substance 3D Designer"],
    },
    AdobeProductSpec {
        id: AdobeProductId::CreativeCloudDesktop,
        name: "Adobe Creative Cloud",
        family: "Adobe Creative Cloud",
        release_version: "6.9",
        min_cc_2027_version: (6, 9),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Creative Cloud\ACC",
        windows_exe: r"Adobe Creative Cloud\ACC\Creative Cloud.exe",
        legacy_windows_exes: &[r"Adobe Creative Cloud\ACC\Creative Cloud Helper.exe"],
        legacy_windows_dirs: &[r"Adobe Creative Cloud"],
        mac_bundles: &["Utilities/Adobe Creative Cloud/ACC/Creative Cloud.app", "Adobe Creative Cloud/Adobe Creative Cloud.app"],
        registry_prefixes: &["Adobe Creative Cloud"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeFirefly,
        name: "Adobe Firefly",
        family: "Adobe Firefly",
        release_version: "2027.0",
        min_cc_2027_version: (2027, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Firefly",
        windows_exe: r"Adobe Firefly\Adobe Firefly.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Firefly/Adobe Firefly.app"],
        registry_prefixes: &["Adobe Firefly"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeExpress,
        name: "Adobe Express",
        family: "Adobe Express",
        release_version: "6.11",
        min_cc_2027_version: (6, 11),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Express",
        windows_exe: r"Adobe Express\Adobe Express.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Express.app", "Adobe Express/Adobe Express.app"],
        registry_prefixes: &["Adobe Express"],
    },
    AdobeProductSpec {
        id: AdobeProductId::LightroomCc,
        name: "Adobe Lightroom",
        family: "Adobe Lightroom",
        release_version: "9.6",
        min_cc_2027_version: (9, 6),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Lightroom CC",
        windows_exe: r"Adobe Lightroom CC\lightroom.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Lightroom CC/Adobe Lightroom.app"],
        registry_prefixes: &["Adobe Lightroom CC", "Adobe Lightroom"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeBridge,
        name: "Adobe Bridge",
        family: "Adobe Bridge",
        release_version: "17.0",
        min_cc_2027_version: (17, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Bridge 2027",
        windows_exe: r"Adobe Bridge 2027\Bridge.exe",
        legacy_windows_exes: &[r"Adobe Bridge 2026\Bridge.exe", r"Adobe Bridge 2025\Bridge.exe"],
        legacy_windows_dirs: &[r"Adobe Bridge 2026", r"Adobe Bridge 2025"],
        mac_bundles: &["Adobe Bridge 2027/Adobe Bridge 2027.app", "Adobe Bridge 2026/Adobe Bridge 2026.app"],
        registry_prefixes: &["Adobe Bridge 2027", "Adobe Bridge"],
    },
    AdobeProductSpec {
        id: AdobeProductId::InCopy2027,
        name: "Adobe InCopy 2027",
        family: "Adobe InCopy",
        release_version: "22.0",
        min_cc_2027_version: (22, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe InCopy 2027",
        windows_exe: r"Adobe InCopy 2027\InCopy.exe",
        legacy_windows_exes: &[r"Adobe InCopy 2026\InCopy.exe", r"Adobe InCopy 2025\InCopy.exe"],
        legacy_windows_dirs: &[r"Adobe InCopy 2026", r"Adobe InCopy 2025"],
        mac_bundles: &["Adobe InCopy 2027/Adobe InCopy 2027.app", "Adobe InCopy 2026/Adobe InCopy 2026.app"],
        registry_prefixes: &["Adobe InCopy 2027", "Adobe InCopy"],
    },
    AdobeProductSpec {
        id: AdobeProductId::CharacterAnimator,
        name: "Adobe Character Animator",
        family: "Adobe Character Animator",
        release_version: "26.0",
        min_cc_2027_version: (26, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Character Animator 2027",
        windows_exe: r"Adobe Character Animator 2027\Support Files\Character Animator.exe",
        legacy_windows_exes: &[
            r"Adobe Character Animator 2026\Support Files\Character Animator.exe",
            r"Adobe Character Animator 2025\Support Files\Character Animator.exe",
        ],
        legacy_windows_dirs: &[r"Adobe Character Animator 2026", r"Adobe Character Animator 2025"],
        mac_bundles: &["Adobe Character Animator 2027/Adobe Character Animator.app", "Adobe Character Animator 2026/Adobe Character Animator.app"],
        registry_prefixes: &["Adobe Character Animator 2027", "Adobe Character Animator"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Substance3DStager,
        name: "Adobe Substance 3D Stager",
        family: "Adobe Substance 3D Stager",
        release_version: "3.1",
        min_cc_2027_version: (3, 1),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Substance 3D Stager",
        windows_exe: r"Adobe Substance 3D Stager\Adobe Substance 3D Stager.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Substance 3D Stager/Adobe Substance 3D Stager.app"],
        registry_prefixes: &["Adobe Substance 3D Stager"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Substance3DSampler,
        name: "Adobe Substance 3D Sampler",
        family: "Adobe Substance 3D Sampler",
        release_version: "6.0",
        min_cc_2027_version: (6, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Substance 3D Sampler",
        windows_exe: r"Adobe Substance 3D Sampler\Adobe Substance 3D Sampler.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Substance 3D Sampler/Adobe Substance 3D Sampler.app"],
        registry_prefixes: &["Adobe Substance 3D Sampler"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Substance3DModeler,
        name: "Adobe Substance 3D Modeler",
        family: "Adobe Substance 3D Modeler",
        release_version: "1.22",
        min_cc_2027_version: (1, 22),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Substance 3D Modeler",
        windows_exe: r"Adobe Substance 3D Modeler\Adobe Substance 3D Modeler.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Substance 3D Modeler/Adobe Substance 3D Modeler.app"],
        registry_prefixes: &["Adobe Substance 3D Modeler"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeDimension,
        name: "Adobe Dimension",
        family: "Adobe Dimension",
        release_version: "4.1",
        min_cc_2027_version: (4, 1),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Dimension",
        windows_exe: r"Adobe Dimension\Dimension.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Dimension/Adobe Dimension.app"],
        registry_prefixes: &["Adobe Dimension"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeFresco,
        name: "Adobe Fresco",
        family: "Adobe Fresco",
        release_version: "5.7",
        min_cc_2027_version: (5, 7),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Fresco",
        windows_exe: r"Adobe Fresco\Fresco.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Fresco.app"],
        registry_prefixes: &["Adobe Fresco"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeCapture,
        name: "Adobe Capture",
        family: "Adobe Capture",
        release_version: "2027.0",
        min_cc_2027_version: (2027, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Capture",
        windows_exe: r"Adobe Capture\Adobe Capture.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Capture.app"],
        registry_prefixes: &["Adobe Capture"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeXd,
        name: "Adobe XD",
        family: "Adobe XD",
        release_version: "61.0",
        min_cc_2027_version: (61, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe XD",
        windows_exe: r"Adobe XD\XD.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe XD/Adobe XD.app"],
        registry_prefixes: &["Adobe XD"],
    },
    AdobeProductSpec {
        id: AdobeProductId::Dreamweaver,
        name: "Adobe Dreamweaver",
        family: "Adobe Dreamweaver",
        release_version: "21.8",
        min_cc_2027_version: (21, 8),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Dreamweaver 2021",
        windows_exe: r"Adobe Dreamweaver 2021\Dreamweaver.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Dreamweaver 2021/Adobe Dreamweaver 2021.app"],
        registry_prefixes: &["Adobe Dreamweaver 2021", "Adobe Dreamweaver"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AcrobatDistiller,
        name: "Adobe Acrobat Distiller DC",
        family: "Adobe Acrobat Distiller",
        release_version: "26.2",
        min_cc_2027_version: (26, 2),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Acrobat Distiller DC",
        windows_exe: r"Acrobat DC\Acrobat\acrodist.exe",
        legacy_windows_exes: &[r"Acrobat Distiller DC\acrodist.exe"],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe Acrobat DC/Acrobat Distiller.app"],
        registry_prefixes: &["Adobe Acrobat Distiller DC", "Adobe Acrobat Distiller"],
    },
    AdobeProductSpec {
        id: AdobeProductId::FrameIo,
        name: "Frame.io",
        family: "Frame.io",
        release_version: "4.0",
        min_cc_2027_version: (4, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Frame.io",
        windows_exe: r"Frame.io\Frame.io.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Frame.io.app", "Frame.io/Frame.io.app"],
        registry_prefixes: &["Frame.io", "Adobe Frame.io"],
    },
    AdobeProductSpec {
        id: AdobeProductId::UxpDeveloperTools,
        name: "UXP Developer Tools",
        family: "UXP Developer Tools",
        release_version: "2.4",
        min_cc_2027_version: (2, 4),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe UXP Developer Tools",
        windows_exe: r"Adobe UXP Developer Tools\Adobe UXP Developer Tools.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Adobe UXP Developer Tools/Adobe UXP Developer Tools.app"],
        registry_prefixes: &["Adobe UXP Developer Tools", "UXP Developer Tools"],
    },
    AdobeProductSpec {
        id: AdobeProductId::CreativeCloudLibraries,
        name: "Creative Cloud Libraries",
        family: "Creative Cloud Libraries",
        release_version: "4.20",
        min_cc_2027_version: (4, 20),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Creative Cloud Experience\libs",
        windows_exe: r"Adobe Creative Cloud Experience\CCLibrary.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Utilities/Adobe Creative Cloud Experience/CCLibrary.app"],
        registry_prefixes: &["Creative Cloud Libraries", "Adobe Creative Cloud Libraries"],
    },
    AdobeProductSpec {
        id: AdobeProductId::AdobeFonts,
        name: "Adobe Fonts",
        family: "Adobe Fonts",
        release_version: "1.0",
        min_cc_2027_version: (1, 0),
        cc_2027_lineup: false,
        companion: None,
        uses_common_files: false,
        windows_dir: r"Adobe Creative Cloud\CoreSync",
        windows_exe: r"Adobe Creative Cloud\CoreSync\CoreSync.exe",
        legacy_windows_exes: &[],
        legacy_windows_dirs: &[],
        mac_bundles: &["Utilities/Adobe Sync/CoreSync/Core Sync.app"],
        registry_prefixes: &["Adobe Fonts"],
    },
];

/// Industry counterpart mapping for a Craft Suite or extended ArtCraft application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterpartSpec {
    /// Craft Suite app identifier (`"photocraft"`, `"pdfcraft"`, `"materialcraft"`, ...).
    pub app_id: &'static str,
    /// Primary counterpart display name (`"Adobe Photoshop 2027"`, `"Figma"`, `"Microsoft Word"`).
    pub display_name: &'static str,
    /// Rich summary metadata including CC 2027 release version and companion plug-ins where applicable.
    pub summary_label: &'static str,
    pub vendor: CounterpartVendor,
    /// Paired Adobe products / shared components (empty for non-Adobe counterparts).
    pub adobe_products: &'static [AdobeProductId],
    /// Relative Windows executable paths for non-Adobe counterparts (checked under Program Files
    /// and `%LOCALAPPDATA%\Programs` / `%LOCALAPPDATA%`).
    pub non_adobe_windows_exes: &'static [&'static str],
    /// macOS bundle relative paths under `/Applications` for non-Adobe counterparts.
    pub non_adobe_mac_bundles: &'static [&'static str],
    /// Lowercase search keywords matched by the launcher's search box.
    pub search_terms: &'static [&'static str],
}

impl CounterpartSpec {
    /// Look up counterpart metadata for one of the 16 official Craft Suite apps by its `id`.
    pub fn by_app_id(app_id: &str) -> Option<&'static CounterpartSpec> {
        CRAFT_SUITE_COUNTERPARTS.iter().find(|c| c.app_id == app_id)
    }

    /// Resolved [`AdobeProductSpec`] entries paired with this Craft Suite app.
    pub fn adobe_specs(&self) -> impl Iterator<Item = &'static AdobeProductSpec> + '_ {
        self.adobe_products.iter().copied().map(AdobeProductSpec::by_id)
    }

    /// Whether a lowercase search needle matches this app's counterpart metadata.
    pub fn matches_query(&self, lower_needle: &str) -> bool {
        if lower_needle.is_empty() {
            return true;
        }
        self.display_name.to_ascii_lowercase().contains(lower_needle)
            || self.summary_label.to_ascii_lowercase().contains(lower_needle)
            || self.search_terms.iter().any(|term| term.contains(lower_needle))
            || self
                .adobe_specs()
                .any(|spec| spec.name.to_ascii_lowercase().contains(lower_needle) || spec.family.to_ascii_lowercase().contains(lower_needle))
    }
}

/// Counterpart definitions for all 16 official Craft Suite applications.
pub const CRAFT_SUITE_COUNTERPARTS: [CounterpartSpec; 16] = [
    CounterpartSpec {
        app_id: "photocraft",
        display_name: "Adobe Photoshop 2027",
        summary_label: "Adobe Photoshop 2027 (28.0, Camera Raw 19.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[
            AdobeProductId::Photoshop2027,
            AdobeProductId::CameraRaw,
            AdobeProductId::AdobeFresco,
            AdobeProductId::AdobeCapture,
            AdobeProductId::CreativeCloudLibraries,
        ],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe photoshop",
            "photoshop 2027",
            "photoshop",
            "camera raw",
            "camera raw 19.0",
            "adobe fresco",
            "fresco",
            "adobe capture",
            "creative cloud libraries",
            "cclibs",
            "ase",
            "psd",
            "psb",
            "8bf",
        ],
    },
    CounterpartSpec {
        app_id: "vectorcraft",
        display_name: "Adobe Illustrator 2027",
        summary_label: "Adobe Illustrator 2027 (31.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Illustrator2027, AdobeProductId::AdobeFresco, AdobeProductId::AdobeCapture, AdobeProductId::AdobeFonts],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe illustrator",
            "illustrator 2027",
            "illustrator",
            "adobe fresco",
            "fresco",
            "adobe capture",
            "adobe fonts",
            "ai",
            "eps",
            "svg",
            "aip",
        ],
    },
    CounterpartSpec {
        app_id: "filmcraft",
        display_name: "Adobe Premiere Pro 2027",
        summary_label: "Adobe Premiere Pro 2027 (27.0) & Adobe Media Encoder 2027 (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::PremierePro2027, AdobeProductId::MediaEncoder2027, AdobeProductId::FrameIo],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe premiere pro",
            "premiere pro 2027",
            "premiere pro",
            "premiere",
            "adobe media encoder",
            "media encoder 2027",
            "media encoder",
            "frame.io",
            "frame io",
            "prproj",
            "fcpxml",
            "prm",
            "aex",
        ],
    },
    CounterpartSpec {
        app_id: "lightcraft",
        display_name: "Adobe Lightroom Classic",
        summary_label: "Adobe Lightroom Classic (16.0, Camera Raw 19.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::LightroomClassic, AdobeProductId::CameraRaw, AdobeProductId::LightroomCc, AdobeProductId::AdobeBridge],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe lightroom",
            "lightroom classic",
            "lightroom cc",
            "lightroom",
            "adobe bridge",
            "bridge 2027",
            "bridge",
            "camera raw",
            "camera raw 19.0",
            "dng",
            "cr3",
            "lrcat",
            "8bf",
        ],
    },
    CounterpartSpec {
        app_id: "pdfcraft",
        display_name: "Adobe Acrobat DC",
        summary_label: "Adobe Acrobat DC (26.2)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::AcrobatDc, AdobeProductId::AcrobatDistiller],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe acrobat", "acrobat dc", "acrobat pro", "acrobat distiller", "distiller", "acrobat", "pdf"],
    },
    CounterpartSpec {
        app_id: "effectcraft",
        display_name: "Adobe After Effects 2027",
        summary_label: "Adobe After Effects 2027 (27.0) & Adobe Media Encoder 2027 (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::AfterEffects2027, AdobeProductId::MediaEncoder2027, AdobeProductId::FrameIo],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe after effects",
            "after effects 2027",
            "after effects",
            "adobe media encoder",
            "media encoder 2027",
            "media encoder",
            "frame.io",
            "frame io",
            "aep",
            "motn",
            "aex",
            "prm",
        ],
    },
    CounterpartSpec {
        app_id: "anicraft",
        display_name: "Adobe Animate",
        summary_label: "Adobe Animate (25.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Animate, AdobeProductId::CharacterAnimator],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe animate", "animate", "adobe character animator", "character animator", "flash", "fla", "xfl"],
    },
    CounterpartSpec {
        app_id: "designcraft",
        display_name: "Adobe InDesign 2027",
        summary_label: "Adobe InDesign 2027 (22.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[
            AdobeProductId::InDesign2027,
            AdobeProductId::InCopy2027,
            AdobeProductId::AdobeFonts,
            AdobeProductId::CreativeCloudLibraries,
        ],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe indesign",
            "indesign 2027",
            "indesign",
            "adobe incopy",
            "incopy 2027",
            "incopy",
            "adobe fonts",
            "indd",
            "idml",
            "icml",
        ],
    },
    CounterpartSpec {
        app_id: "figcraft",
        display_name: "Figma",
        summary_label: "Figma",
        vendor: CounterpartVendor::Figma,
        adobe_products: &[AdobeProductId::AdobeXd, AdobeProductId::Dreamweaver],
        non_adobe_windows_exes: &[r"Figma\Figma.exe"],
        non_adobe_mac_bundles: &["Figma.app"],
        search_terms: &["figma", "adobe xd", "xd", "adobe dreamweaver", "dreamweaver", "html", "css"],
    },
    CounterpartSpec {
        app_id: "soundcraft",
        display_name: "Adobe Audition",
        summary_label: "Adobe Audition (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Audition],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe audition", "audition", "pro tools", "daw", "sesx", "wav", "flac"],
    },
    CounterpartSpec {
        app_id: "wordcraft",
        display_name: "Microsoft Word",
        summary_label: "Microsoft Word",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[AdobeProductId::InCopy2027],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\WINWORD.EXE"],
        non_adobe_mac_bundles: &["Microsoft Word.app"],
        search_terms: &["microsoft word", "word", "docx", "adobe incopy", "incopy 2027", "incopy"],
    },
    CounterpartSpec {
        app_id: "deckcraft",
        display_name: "Microsoft PowerPoint",
        summary_label: "Microsoft PowerPoint",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[AdobeProductId::AdobeExpress],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\POWERPNT.EXE"],
        non_adobe_mac_bundles: &["Microsoft PowerPoint.app"],
        search_terms: &["microsoft powerpoint", "powerpoint", "pptx", "adobe express", "express"],
    },
    CounterpartSpec {
        app_id: "cadcraft",
        display_name: "AutoCAD",
        summary_label: "AutoCAD",
        vendor: CounterpartVendor::Autodesk,
        adobe_products: &[AdobeProductId::Substance3DModeler],
        non_adobe_windows_exes: &[r"Autodesk\AutoCAD 2027\acad.exe", r"Autodesk\AutoCAD 2026\acad.exe", r"Autodesk\AutoCAD 2025\acad.exe"],
        non_adobe_mac_bundles: &["Autodesk/AutoCAD 2027/AutoCAD 2027.app", "Autodesk/AutoCAD 2026/AutoCAD 2026.app"],
        search_terms: &["autocad", "autodesk", "adobe substance 3d modeler", "substance 3d modeler", "substance modeler", "dwg", "dxf", "stl"],
    },
    CounterpartSpec {
        app_id: "gridcraft",
        display_name: "Microsoft Excel",
        summary_label: "Microsoft Excel",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\EXCEL.EXE"],
        non_adobe_mac_bundles: &["Microsoft Excel.app"],
        search_terms: &["microsoft excel", "excel", "xlsx", "csv"],
    },
    CounterpartSpec {
        app_id: "texturecraft",
        display_name: "Adobe Substance 3D Painter",
        summary_label: "Adobe Substance 3D Painter (11.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Substance3DPainter, AdobeProductId::Substance3DStager, AdobeProductId::AdobeDimension],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe substance 3d painter",
            "substance 3d painter",
            "substance painter",
            "adobe substance 3d stager",
            "substance 3d stager",
            "substance stager",
            "adobe dimension",
            "dimension",
            "glb",
            "usdz",
        ],
    },
    CounterpartSpec {
        app_id: "materialcraft",
        display_name: "Adobe Substance 3D Designer",
        summary_label: "Adobe Substance 3D Designer (15.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Substance3DDesigner, AdobeProductId::Substance3DSampler],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe substance 3d designer",
            "substance 3d designer",
            "substance designer",
            "adobe substance 3d sampler",
            "substance 3d sampler",
            "substance sampler",
            "sbs",
            "mcmat",
        ],
    },
];

/// Extended counterpart definitions for platform-wide studio, developer, and community apps outside
/// [`CRAFT_SUITE_16_IDS`].
pub const EXTENDED_COUNTERPARTS: [CounterpartSpec; 4] = [
    CounterpartSpec {
        app_id: "artcraft",
        display_name: "Adobe Firefly & Adobe Express",
        summary_label: "Adobe Firefly (2027.0) & Adobe Express (6.11)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::AdobeFirefly, AdobeProductId::AdobeExpress, AdobeProductId::CreativeCloudDesktop],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe firefly", "firefly", "adobe express", "express", "adobe creative cloud", "creative cloud", "generative ai", "gen ai"],
    },
    CounterpartSpec {
        app_id: "craft-anything",
        display_name: "UXP Developer Tools & Extension Manager",
        summary_label: "UXP Developer Tools (2.4) & Extension Manager",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::UxpDeveloperTools, AdobeProductId::CreativeCloudDesktop],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["uxp developer tools", "uxp", "extension manager", "adobe cep", "cep", "creative cloud"],
    },
    CounterpartSpec {
        app_id: "solidcraft",
        display_name: "Autodesk Fusion & SolidWorks",
        summary_label: "Autodesk Fusion & SolidWorks",
        vendor: CounterpartVendor::Autodesk,
        adobe_products: &[AdobeProductId::Substance3DModeler],
        non_adobe_windows_exes: &[r"SOLIDWORKS Corp\SOLIDWORKS\SLDWORKS.exe"],
        non_adobe_mac_bundles: &["Autodesk Fusion.app"],
        search_terms: &["autodesk fusion", "fusion 360", "solidworks", "parametric cad", "step", "iges"],
    },
    CounterpartSpec {
        app_id: "mailcraft",
        display_name: "Microsoft Outlook",
        summary_label: "Microsoft Outlook",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\OUTLOOK.EXE"],
        non_adobe_mac_bundles: &["Microsoft Outlook.app"],
        search_terms: &["microsoft outlook", "outlook", "eml", "mbox", "pst", "ics", "vcf"],
    },
];

/// Look up counterpart metadata across all 16 Craft Suite apps plus extended studio, developer, and
/// community apps (`"artcraft"`, `"craft-anything"`, `"solidcraft"`, `"mailcraft"`).
pub fn extended_counterpart_by_app_id(app_id: &str) -> Option<&'static CounterpartSpec> {
    CounterpartSpec::by_app_id(app_id).or_else(|| EXTENDED_COUNTERPARTS.iter().find(|c| c.app_id == app_id))
}

/// Iterator over the primary Adobe Creative Cloud 2027 & Acrobat DC 26.2 product specifications.
pub fn adobe_cc_2027_specs() -> impl Iterator<Item = &'static AdobeProductSpec> {
    ADOBE_PRODUCTS.iter().filter(|p| p.cc_2027_lineup)
}

/// Creative Cloud Desktop (`KCCC v6.9.1`) domain category filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CreativeCategory {
    #[default]
    All,
    Photo,
    VideoAudio,
    Design,
    DocumentPdf,
    ThreeD,
    GenAi,
}

impl CreativeCategory {
    /// All Creative Cloud Desktop category tabs in display order.
    pub const ALL: [Self; 7] = [Self::All, Self::Photo, Self::VideoAudio, Self::Design, Self::DocumentPdf, Self::ThreeD, Self::GenAi];

    /// Canonical English UI label for this category.
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All Apps",
            Self::Photo => "Photo",
            Self::VideoAudio => "Video & Audio",
            Self::Design => "Design",
            Self::DocumentPdf => "Document & PDF",
            Self::ThreeD => "3D & Material",
            Self::GenAi => "Gen AI",
        }
    }
}

/// Creative Cloud Desktop domain categories assigned to a Craft app by `app_id`.
pub fn categories_for_app(app_id: &str) -> &'static [CreativeCategory] {
    match app_id {
        "artcraft" => &[CreativeCategory::GenAi, CreativeCategory::Photo, CreativeCategory::VideoAudio, CreativeCategory::Design],
        "photocraft" => &[CreativeCategory::Photo, CreativeCategory::Design, CreativeCategory::GenAi],
        "lightcraft" => &[CreativeCategory::Photo],
        "vectorcraft" => &[CreativeCategory::Design, CreativeCategory::GenAi],
        "designcraft" => &[CreativeCategory::Design, CreativeCategory::DocumentPdf],
        "figcraft" => &[CreativeCategory::Design],
        "filmcraft" => &[CreativeCategory::VideoAudio],
        "effectcraft" => &[CreativeCategory::VideoAudio, CreativeCategory::ThreeD],
        "anicraft" => &[CreativeCategory::VideoAudio, CreativeCategory::Design],
        "soundcraft" => &[CreativeCategory::VideoAudio],
        "pdfcraft" => &[CreativeCategory::DocumentPdf],
        "wordcraft" => &[CreativeCategory::DocumentPdf],
        "deckcraft" => &[CreativeCategory::DocumentPdf, CreativeCategory::Design],
        "gridcraft" => &[CreativeCategory::DocumentPdf],
        "mailcraft" => &[CreativeCategory::DocumentPdf],
        "texturecraft" => &[CreativeCategory::ThreeD],
        "materialcraft" => &[CreativeCategory::ThreeD],
        "cadcraft" => &[CreativeCategory::ThreeD],
        "solidcraft" => &[CreativeCategory::ThreeD],
        "craft-anything" => &[CreativeCategory::GenAi],
        _ => &[],
    }
}

/// Whether `app_id` belongs to `category` (`CreativeCategory::All` matches every app).
pub fn app_in_category(app_id: &str, category: CreativeCategory) -> bool {
    category == CreativeCategory::All || categories_for_app(app_id).contains(&category)
}

fn normalize_ext_token(ext: &str) -> String {
    let trimmed = ext.trim().trim_matches(['"', '\'']);
    let token = match trimmed.rsplit_once('.') {
        Some((_, suffix)) => suffix,
        None => trimmed,
    };
    token.trim().to_ascii_lowercase()
}

/// Route a file extension (`".psd"`, `"PSD"`, `".8bf"`, `".aip"`, `".prm"`, `".aex"`, ...) to the
/// primary canonical Craft Suite `app_id`.
pub fn route_file_extension(ext: &str) -> Option<&'static str> {
    let lower = normalize_ext_token(ext);
    match lower.as_str() {
        // PhotoCraft: raster documents, layered files, swatches, C++ .8bf filter plug-ins, and CC Libraries
        "psd" | "psb" | "pcraft" | "ase" | "aco" | "cclibs" | "cclib" | "ora" | "kra" | "xcf" | "pdn" | "abr" | "asl" | "atn" | "8bf" => {
            Some("photocraft")
        }
        // LightCraft: camera raw and photo catalogs
        "raw" | "cr3" | "cr2" | "dng" | "lrcat" | "nef" | "arw" | "raf" | "orf" | "rw2" | "pef" | "xmp" => Some("lightcraft"),
        // VectorCraft: vector illustration, artwork, and C++ .aip SweetPea/PICA plug-ins
        "ai" | "eps" | "svg" | "svgz" | "vcraft" | "aip" => Some("vectorcraft"),
        // FilmCraft: non-linear video editing timelines, interchange, and C++ .prm plug-ins
        "fcproj" | "prproj" | "fcpxml" | "mp4" | "mov" | "mxf" | "mkv" | "prm" => Some("filmcraft"),
        // EffectCraft: motion graphics, compositing, and C++ .aex plug-ins
        "ecproj" | "aep" | "aepx" | "motn" | "aex" => Some("effectcraft"),
        // DesignCraft: page layout, editorial stories, and publishing
        "indd" | "indt" | "idml" | "icml" | "designcraft" | "dcdoc" => Some("designcraft"),
        // PdfCraft: PDF documents and forms
        "pdf" | "fdf" | "xfdf" => Some("pdfcraft"),
        // AniCraft: 2D timeline vector animation
        "xfl" | "fla" | "anicraft" | "swf" => Some("anicraft"),
        // SoundCraft: DAW sessions and audio
        "sesx" | "wav" | "flac" | "scraft" | "aiff" | "aif" | "mp3" | "ogg" | "mid" | "midi" => Some("soundcraft"),
        // MaterialCraft: procedural materials
        "sbs" | "sbsar" | "mcmat" | "mtlx" => Some("materialcraft"),
        // TextureCraft: 3D texture painting and scene staging
        "tcraft" | "glb" | "gltf" | "usdz" | "spp" | "obj" | "fbx" => Some("texturecraft"),
        // FigCraft: UI/UX design, web layouts, and prototypes
        "fig" | "figcraft" | "xd" | "html" | "htm" | "css" | "sketch" | "penpot" => Some("figcraft"),
        // CADCraft: 2D/3D CAD drafting and meshes
        "dwg" | "dxf" | "stl" | "step" | "stp" | "iges" | "igs" => Some("cadcraft"),
        // WordCraft: word processing
        "docx" | "doc" | "rtf" | "odt" => Some("wordcraft"),
        // DeckCraft: slide decks and presentations
        "pptx" | "ppt" | "odp" => Some("deckcraft"),
        // GridCraft: spreadsheets and tabular data
        "xlsx" | "xls" | "xlsm" | "ods" | "csv" | "tsv" => Some("gridcraft"),
        // MailCraft: mail, calendar, and contact archives
        "eml" | "mbox" | "pst" | "ics" | "vcf" => Some("mailcraft"),
        _ => None,
    }
}

/// Secondary Craft Suite host `app_id`s that also load a shared third-party C++ plug-in extension
/// (`.8bf` -> `lightcraft`, `.prm` -> `effectcraft`, `.aex` -> `filmcraft`).
pub fn route_secondary_file_extensions(ext: &str) -> &'static [&'static str] {
    match normalize_ext_token(ext).as_str() {
        "8bf" => &["lightcraft"],
        "prm" => &["effectcraft"],
        "aex" => &["filmcraft"],
        _ => &[],
    }
}

/// Route a filesystem path (or bare `".ext"` / `"filename.ext"`) to its primary target [`crate::CraftApp`].
pub fn route_file_path(path: &Path) -> Option<&'static crate::CraftApp> {
    let ext = path.extension().and_then(|e| e.to_str()).or_else(|| path.file_name().and_then(|n| n.to_str()).and_then(|s| s.strip_prefix('.')));
    let app_id = route_file_extension(ext?)?;
    crate::CraftApp::by_id(app_id)
}

/// Secondary [`crate::CraftApp`] hosts for a filesystem path when the file format is shared across
/// Craft hosts (such as `.8bf`, `.prm`, and `.aex` C++ binary plug-ins).
pub fn route_secondary_file_paths(path: &Path) -> Vec<&'static crate::CraftApp> {
    let Some(ext) = path.extension().and_then(|e| e.to_str()).or_else(|| path.file_name().and_then(|n| n.to_str()).and_then(|s| s.strip_prefix('.')))
    else {
        return Vec::new();
    };
    route_secondary_file_extensions(ext).iter().copied().filter_map(crate::CraftApp::by_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CraftApp, SUITE};

    #[test]
    fn all_sixteen_craft_suite_apps_are_in_catalog_and_counterparts() {
        assert_eq!(CRAFT_SUITE_16_IDS.len(), 16);
        assert_eq!(CRAFT_SUITE_COUNTERPARTS.len(), 16);
        for id in CRAFT_SUITE_16_IDS {
            let app = CraftApp::by_id(id).unwrap_or_else(|| panic!("missing Craft Suite app {id} in SUITE"));
            assert_eq!(app.kind, crate::catalog::AppKind::Official, "{id} should be official");
            let spec = CounterpartSpec::by_app_id(id).unwrap_or_else(|| panic!("missing CounterpartSpec for {id}"));
            assert!(!spec.display_name.is_empty(), "{id}");
            assert!(!spec.summary_label.is_empty(), "{id}");
            assert!(SUITE.iter().any(|a| a.id == id), "{id} not in SUITE");
        }
    }

    #[test]
    fn adobe_cc_2027_specs_match_exact_versions_and_windows_paths() {
        let ps = AdobeProductSpec::by_id(AdobeProductId::Photoshop2027);
        assert_eq!(ps.name, "Adobe Photoshop 2027");
        assert_eq!(ps.release_version, "28.0");
        assert_eq!(ps.windows_exe, r"Adobe Photoshop 2027\Photoshop.exe");
        assert_eq!(ps.companion, Some(("Camera Raw", "19.0")));
        assert_eq!(ps.display_metadata(), "Adobe Photoshop 2027 28.0 (Camera Raw 19.0)");

        let acr = AdobeProductSpec::by_id(AdobeProductId::CameraRaw);
        assert_eq!(acr.release_version, "19.0");
        assert!(acr.uses_common_files);
        assert!(acr.windows_exe.ends_with(r"Camera Raw.8bi"));

        let ai = AdobeProductSpec::by_id(AdobeProductId::Illustrator2027);
        assert_eq!(ai.name, "Adobe Illustrator 2027");
        assert_eq!(ai.release_version, "31.0");
        assert_eq!(ai.windows_exe, r"Adobe Illustrator 2027\Support Files\Contents\Windows\Illustrator.exe");

        let pr = AdobeProductSpec::by_id(AdobeProductId::PremierePro2027);
        assert_eq!(pr.name, "Adobe Premiere Pro 2027");
        assert_eq!(pr.release_version, "27.0");
        assert_eq!(pr.windows_exe, r"Adobe Premiere Pro 2027\Adobe Premiere Pro.exe");

        let ame = AdobeProductSpec::by_id(AdobeProductId::MediaEncoder2027);
        assert_eq!(ame.name, "Adobe Media Encoder 2027");
        assert_eq!(ame.release_version, "27.0");
        assert_eq!(ame.windows_exe, r"Adobe Media Encoder 2027\Adobe Media Encoder.exe");

        let ae = AdobeProductSpec::by_id(AdobeProductId::AfterEffects2027);
        assert_eq!(ae.name, "Adobe After Effects 2027");
        assert_eq!(ae.release_version, "27.0");
        assert_eq!(ae.windows_dir, r"Adobe After Effects 2027");
        assert_eq!(ae.windows_exe, r"Adobe After Effects 2027\Support Files\AfterFX.exe");

        let id = AdobeProductSpec::by_id(AdobeProductId::InDesign2027);
        assert_eq!(id.name, "Adobe InDesign 2027");
        assert_eq!(id.release_version, "22.0");
        assert_eq!(id.windows_dir, r"Adobe InDesign 2027");

        let acrobat = AdobeProductSpec::by_id(AdobeProductId::AcrobatDc);
        assert_eq!(acrobat.name, "Adobe Acrobat DC");
        assert_eq!(acrobat.release_version, "26.2");
        assert_eq!(acrobat.windows_exe, r"Acrobat DC\Acrobat\Acrobat.exe");

        assert_eq!(adobe_cc_2027_specs().count(), 8);
    }

    #[test]
    fn full_creative_cloud_desktop_kccc_catalog_has_all_33_products_with_valid_versions() {
        assert_eq!(ADOBE_PRODUCTS.len(), 33);
        let mut seen = std::collections::HashSet::new();
        for spec in &ADOBE_PRODUCTS {
            assert!(seen.insert(spec.id), "duplicate AdobeProductId {:?}", spec.id);
            assert_eq!(AdobeProductSpec::by_id(spec.id), spec);
            assert!(spec.parsed_release_version().is_some(), "unparseable release_version for {:?}", spec.id);
            assert!(!spec.windows_dir.is_empty(), "empty windows_dir for {:?}", spec.id);
            assert!(!spec.windows_exe.is_empty(), "empty windows_exe for {:?}", spec.id);
        }

        let kccc = AdobeProductSpec::by_id(AdobeProductId::CreativeCloudDesktop);
        assert_eq!(kccc.name, "Adobe Creative Cloud");
        assert_eq!(kccc.release_version, "6.9");
        assert!(!kccc.cc_2027_lineup);
        assert_eq!(kccc.windows_exe, r"Adobe Creative Cloud\ACC\Creative Cloud.exe");

        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeFirefly).release_version, "2027.0");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeExpress).release_version, "6.11");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::LightroomCc).windows_exe, r"Adobe Lightroom CC\lightroom.exe");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeBridge).windows_exe, r"Adobe Bridge 2027\Bridge.exe");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::InCopy2027).windows_exe, r"Adobe InCopy 2027\InCopy.exe");
        assert_eq!(
            AdobeProductSpec::by_id(AdobeProductId::CharacterAnimator).windows_exe,
            r"Adobe Character Animator 2027\Support Files\Character Animator.exe"
        );
        assert_eq!(
            AdobeProductSpec::by_id(AdobeProductId::Substance3DStager).windows_exe,
            r"Adobe Substance 3D Stager\Adobe Substance 3D Stager.exe"
        );
        assert_eq!(
            AdobeProductSpec::by_id(AdobeProductId::Substance3DSampler).windows_exe,
            r"Adobe Substance 3D Sampler\Adobe Substance 3D Sampler.exe"
        );
        assert_eq!(
            AdobeProductSpec::by_id(AdobeProductId::Substance3DModeler).windows_exe,
            r"Adobe Substance 3D Modeler\Adobe Substance 3D Modeler.exe"
        );
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeDimension).windows_exe, r"Adobe Dimension\Dimension.exe");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeFresco).release_version, "5.7");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeCapture).release_version, "2027.0");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeXd).release_version, "61.0");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::Dreamweaver).windows_exe, r"Adobe Dreamweaver 2021\Dreamweaver.exe");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AcrobatDistiller).release_version, "26.2");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::FrameIo).release_version, "4.0");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::UxpDeveloperTools).release_version, "2.4");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::CreativeCloudLibraries).release_version, "4.20");
        assert_eq!(AdobeProductSpec::by_id(AdobeProductId::AdobeFonts).release_version, "1.0");
    }

    #[test]
    fn extended_counterparts_and_companion_products_resolve_cleanly() {
        let artcraft = extended_counterpart_by_app_id("artcraft").unwrap();
        assert_eq!(artcraft.display_name, "Adobe Firefly & Adobe Express");
        assert!(artcraft.matches_query("firefly"));
        assert!(artcraft.matches_query("adobe express"));

        let craft_anything = extended_counterpart_by_app_id("craft-anything").unwrap();
        assert_eq!(craft_anything.display_name, "UXP Developer Tools & Extension Manager");
        assert!(craft_anything.matches_query("uxp developer tools"));
        assert!(craft_anything.matches_query("extension manager"));

        assert!(extended_counterpart_by_app_id("solidcraft").is_some());
        assert!(extended_counterpart_by_app_id("mailcraft").is_some());

        // Companion pairings across the 16 official Craft Suite counterparts:
        assert!(CounterpartSpec::by_app_id("designcraft").unwrap().adobe_products.contains(&AdobeProductId::InCopy2027));
        assert!(CounterpartSpec::by_app_id("wordcraft").unwrap().adobe_products.contains(&AdobeProductId::InCopy2027));
        assert!(CounterpartSpec::by_app_id("anicraft").unwrap().adobe_products.contains(&AdobeProductId::CharacterAnimator));
        assert!(CounterpartSpec::by_app_id("lightcraft").unwrap().adobe_products.contains(&AdobeProductId::LightroomCc));
        assert!(CounterpartSpec::by_app_id("lightcraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeBridge));
        assert!(CounterpartSpec::by_app_id("texturecraft").unwrap().adobe_products.contains(&AdobeProductId::Substance3DStager));
        assert!(CounterpartSpec::by_app_id("texturecraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeDimension));
        assert!(CounterpartSpec::by_app_id("materialcraft").unwrap().adobe_products.contains(&AdobeProductId::Substance3DSampler));
        assert!(CounterpartSpec::by_app_id("cadcraft").unwrap().adobe_products.contains(&AdobeProductId::Substance3DModeler));
        assert!(CounterpartSpec::by_app_id("figcraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeXd));
        assert!(CounterpartSpec::by_app_id("figcraft").unwrap().adobe_products.contains(&AdobeProductId::Dreamweaver));
        assert!(CounterpartSpec::by_app_id("photocraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeFresco));
        assert!(CounterpartSpec::by_app_id("photocraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeCapture));
        assert!(CounterpartSpec::by_app_id("vectorcraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeFresco));
        assert!(CounterpartSpec::by_app_id("vectorcraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeCapture));
        assert!(CounterpartSpec::by_app_id("filmcraft").unwrap().adobe_products.contains(&AdobeProductId::FrameIo));
        assert!(CounterpartSpec::by_app_id("effectcraft").unwrap().adobe_products.contains(&AdobeProductId::FrameIo));
        assert!(CounterpartSpec::by_app_id("deckcraft").unwrap().adobe_products.contains(&AdobeProductId::AdobeExpress));
        assert!(CounterpartSpec::by_app_id("pdfcraft").unwrap().adobe_products.contains(&AdobeProductId::AcrobatDistiller));
    }

    #[test]
    fn creative_categories_cover_every_suite_app_and_domain() {
        for app in &SUITE {
            let cats = categories_for_app(app.id);
            assert!(!cats.is_empty(), "{} has no CreativeCategory", app.id);
            assert!(app.in_category(CreativeCategory::All));
            for &cat in cats {
                assert!(app.in_category(cat));
            }
        }
        assert!(app_in_category("artcraft", CreativeCategory::GenAi));
        assert_eq!(CreativeCategory::ALL.len(), 7);
        for cat in CreativeCategory::ALL {
            assert!(!cat.label().is_empty());
        }
    }

    #[test]
    fn cross_app_file_router_maps_all_creative_formats_to_suite_apps() {
        let cases = [
            (".psd", "photocraft"),
            (".psb", "photocraft"),
            (".pcraft", "photocraft"),
            (".ase", "photocraft"),
            (".cclibs", "photocraft"),
            (".raw", "lightcraft"),
            (".cr3", "lightcraft"),
            (".dng", "lightcraft"),
            (".lrcat", "lightcraft"),
            (".ai", "vectorcraft"),
            (".eps", "vectorcraft"),
            (".svg", "vectorcraft"),
            (".vcraft", "vectorcraft"),
            (".fcproj", "filmcraft"),
            (".prproj", "filmcraft"),
            (".fcpxml", "filmcraft"),
            (".ecproj", "effectcraft"),
            (".aep", "effectcraft"),
            (".motn", "effectcraft"),
            (".indd", "designcraft"),
            (".idml", "designcraft"),
            (".icml", "designcraft"),
            (".pdf", "pdfcraft"),
            (".xfl", "anicraft"),
            (".fla", "anicraft"),
            (".sesx", "soundcraft"),
            (".wav", "soundcraft"),
            (".flac", "soundcraft"),
            (".sbs", "materialcraft"),
            (".mcmat", "materialcraft"),
            (".tcraft", "texturecraft"),
            (".glb", "texturecraft"),
            (".usdz", "texturecraft"),
            (".fig", "figcraft"),
            (".xd", "figcraft"),
            (".html", "figcraft"),
            (".css", "figcraft"),
            (".dwg", "cadcraft"),
            (".dxf", "cadcraft"),
            (".stl", "cadcraft"),
            (".docx", "wordcraft"),
            (".pptx", "deckcraft"),
            (".xlsx", "gridcraft"),
        ];
        for (ext, expected_id) in cases {
            assert_eq!(route_file_extension(ext), Some(expected_id), "extension {ext}");
            assert_eq!(route_file_extension(&ext.to_ascii_uppercase()), Some(expected_id), "upper {ext}");
            let sample_path = Path::new("/tmp").join(format!("project{ext}"));
            let routed = route_file_path(&sample_path).unwrap_or_else(|| panic!("route_file_path failed for {ext}"));
            assert_eq!(routed.id, expected_id, "path {sample_path:?}");
        }
        assert_eq!(route_file_extension("unknown_ext_xyz"), None);
        assert!(route_file_path(Path::new("/tmp/README")).is_none());
    }

    #[test]
    fn binary_cpp_plugin_extensions_route_to_primary_and_secondary_craft_hosts() {
        let plugin_cases: &[(&str, &str, &[&str])] = &[
            (".8bf", "photocraft", &["lightcraft"]),
            (".prm", "filmcraft", &["effectcraft"]),
            (".aex", "effectcraft", &["filmcraft"]),
            (".aip", "vectorcraft", &[]),
        ];
        for &(ext, primary, expected_secondary) in plugin_cases {
            assert_eq!(route_file_extension(ext), Some(primary), "primary {ext}");
            assert_eq!(route_file_extension(&ext.to_ascii_uppercase()), Some(primary), "upper {ext}");
            assert_eq!(route_secondary_file_extensions(ext), expected_secondary, "secondary {ext}");

            let p = Path::new("/plugins").join(format!("SamplePlugin{ext}"));
            assert_eq!(route_file_path(&p).map(|a| a.id), Some(primary));
            let sec_ids: Vec<&str> = route_secondary_file_paths(&p).into_iter().map(|a| a.id).collect();
            assert_eq!(sec_ids, expected_secondary);

            let needle = ext.trim_start_matches('.');
            assert!(CounterpartSpec::by_app_id(primary).unwrap().matches_query(needle), "{primary} search term {needle}");
            for &sec in expected_secondary {
                assert!(CounterpartSpec::by_app_id(sec).unwrap().matches_query(needle), "{sec} search term {needle}");
            }
        }
    }

    #[test]
    fn version_baseline_checks_distinguish_cc_2027_and_older_releases() {
        let ps = AdobeProductSpec::by_id(AdobeProductId::Photoshop2027);
        assert!(ps.matches_cc_2027_version(&version::parse("28.0.0").unwrap()));
        assert!(ps.matches_cc_2027_version(&version::parse("28.2.1").unwrap()));
        assert!(!ps.matches_cc_2027_version(&version::parse("27.4.0").unwrap()));

        let acrobat = AdobeProductSpec::by_id(AdobeProductId::AcrobatDc);
        assert!(acrobat.matches_cc_2027_version(&version::parse("26.2.20050").unwrap()));
        assert!(!acrobat.matches_cc_2027_version(&version::parse("26.1.0").unwrap()));
        assert!(!acrobat.matches_cc_2027_version(&version::parse("25.0.0").unwrap()));
    }

    #[test]
    fn counterpart_search_matches_adobe_2027_and_new_suite_apps() {
        assert!(CounterpartSpec::by_app_id("photocraft").unwrap().matches_query("photoshop 2027"));
        assert!(CounterpartSpec::by_app_id("photocraft").unwrap().matches_query("camera raw 19.0"));
        assert!(CounterpartSpec::by_app_id("photocraft").unwrap().matches_query("adobe fresco"));
        assert!(CounterpartSpec::by_app_id("filmcraft").unwrap().matches_query("media encoder 2027"));
        assert!(CounterpartSpec::by_app_id("filmcraft").unwrap().matches_query("frame.io"));
        assert!(CounterpartSpec::by_app_id("pdfcraft").unwrap().matches_query("acrobat dc"));
        assert!(CounterpartSpec::by_app_id("pdfcraft").unwrap().matches_query("acrobat distiller"));
        assert!(CounterpartSpec::by_app_id("anicraft").unwrap().matches_query("adobe animate"));
        assert!(CounterpartSpec::by_app_id("anicraft").unwrap().matches_query("character animator"));
        assert!(CounterpartSpec::by_app_id("designcraft").unwrap().matches_query("incopy 2027"));
        assert!(CounterpartSpec::by_app_id("lightcraft").unwrap().matches_query("adobe bridge"));
        assert!(CounterpartSpec::by_app_id("texturecraft").unwrap().matches_query("substance 3d painter"));
        assert!(CounterpartSpec::by_app_id("texturecraft").unwrap().matches_query("substance 3d stager"));
        assert!(CounterpartSpec::by_app_id("materialcraft").unwrap().matches_query("substance 3d designer"));
        assert!(CounterpartSpec::by_app_id("materialcraft").unwrap().matches_query("substance 3d sampler"));
        assert!(CraftApp::by_id("craft-anything").unwrap().matches_counterpart_query("uxp developer tools"));
    }
}
