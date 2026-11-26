//! Industry counterpart metadata and Adobe Creative Cloud 2027 product definitions for the 16
//! Craft Suite apps.
//!
//! Kept separate from the Ed25519-signed `catalog.toml` schema so the hosted startup snapshot
//! verification remains untouched while `core`, `native` (`detect.rs`), and `ui-egui` share a
//! single source of truth for counterpart display metadata, search terms, expected versions, and
//! platform install paths.

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

/// Identifier for an Adobe desktop product or shared plug-in tracked by counterpart detection.
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
    /// Whether this entry is part of the primary Adobe CC 2027 / Acrobat DC 26.2 lineup.
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

/// All Adobe products and shared components recognized by counterpart detection.
pub const ADOBE_PRODUCTS: [AdobeProductSpec; 13] = [
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
        legacy_windows_exes: &[
            r"Adobe Photoshop 2026\Photoshop.exe",
            r"Adobe Photoshop 2025\Photoshop.exe",
            r"Adobe Photoshop 2024\Photoshop.exe",
        ],
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
        legacy_windows_exes: &[
            r"Adobe InDesign 2026\InDesign.exe",
            r"Adobe InDesign 2025\InDesign.exe",
            r"Adobe InDesign 2024\InDesign.exe",
        ],
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
];

/// Industry counterpart mapping for one of the 16 Craft Suite apps.
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
    /// Look up counterpart metadata for a Craft Suite app by its `id`.
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
            || self.adobe_specs().any(|spec| {
                spec.name.to_ascii_lowercase().contains(lower_needle)
                    || spec.family.to_ascii_lowercase().contains(lower_needle)
            })
    }
}

/// Counterpart definitions for all 16 official Craft Suite applications.
pub const CRAFT_SUITE_COUNTERPARTS: [CounterpartSpec; 16] = [
    CounterpartSpec {
        app_id: "photocraft",
        display_name: "Adobe Photoshop 2027",
        summary_label: "Adobe Photoshop 2027 (28.0, Camera Raw 19.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Photoshop2027, AdobeProductId::CameraRaw],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe photoshop", "photoshop 2027", "photoshop", "camera raw", "camera raw 19.0", "psd", "psb"],
    },
    CounterpartSpec {
        app_id: "vectorcraft",
        display_name: "Adobe Illustrator 2027",
        summary_label: "Adobe Illustrator 2027 (31.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Illustrator2027],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe illustrator", "illustrator 2027", "illustrator"],
    },
    CounterpartSpec {
        app_id: "filmcraft",
        display_name: "Adobe Premiere Pro 2027",
        summary_label: "Adobe Premiere Pro 2027 (27.0) & Adobe Media Encoder 2027 (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::PremierePro2027, AdobeProductId::MediaEncoder2027],
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
        ],
    },
    CounterpartSpec {
        app_id: "lightcraft",
        display_name: "Adobe Lightroom Classic",
        summary_label: "Adobe Lightroom Classic (16.0, Camera Raw 19.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::LightroomClassic, AdobeProductId::CameraRaw],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe lightroom", "lightroom classic", "lightroom", "camera raw", "camera raw 19.0"],
    },
    CounterpartSpec {
        app_id: "pdfcraft",
        display_name: "Adobe Acrobat DC",
        summary_label: "Adobe Acrobat DC (26.2)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::AcrobatDc],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe acrobat", "acrobat dc", "acrobat pro", "acrobat"],
    },
    CounterpartSpec {
        app_id: "effectcraft",
        display_name: "Adobe After Effects 2027",
        summary_label: "Adobe After Effects 2027 (27.0) & Adobe Media Encoder 2027 (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::AfterEffects2027, AdobeProductId::MediaEncoder2027],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &[
            "adobe after effects",
            "after effects 2027",
            "after effects",
            "adobe media encoder",
            "media encoder 2027",
            "media encoder",
        ],
    },
    CounterpartSpec {
        app_id: "anicraft",
        display_name: "Adobe Animate",
        summary_label: "Adobe Animate (25.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Animate],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe animate", "animate", "flash", "fla", "xfl"],
    },
    CounterpartSpec {
        app_id: "designcraft",
        display_name: "Adobe InDesign 2027",
        summary_label: "Adobe InDesign 2027 (22.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::InDesign2027],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe indesign", "indesign 2027", "indesign", "indd", "idml"],
    },
    CounterpartSpec {
        app_id: "figcraft",
        display_name: "Figma",
        summary_label: "Figma",
        vendor: CounterpartVendor::Figma,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Figma\Figma.exe"],
        non_adobe_mac_bundles: &["Figma.app"],
        search_terms: &["figma", "adobe xd"],
    },
    CounterpartSpec {
        app_id: "soundcraft",
        display_name: "Adobe Audition",
        summary_label: "Adobe Audition (27.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Audition],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe audition", "audition", "pro tools", "daw"],
    },
    CounterpartSpec {
        app_id: "wordcraft",
        display_name: "Microsoft Word",
        summary_label: "Microsoft Word",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\WINWORD.EXE"],
        non_adobe_mac_bundles: &["Microsoft Word.app"],
        search_terms: &["microsoft word", "word", "docx"],
    },
    CounterpartSpec {
        app_id: "deckcraft",
        display_name: "Microsoft PowerPoint",
        summary_label: "Microsoft PowerPoint",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\POWERPNT.EXE"],
        non_adobe_mac_bundles: &["Microsoft PowerPoint.app"],
        search_terms: &["microsoft powerpoint", "powerpoint", "pptx"],
    },
    CounterpartSpec {
        app_id: "cadcraft",
        display_name: "AutoCAD",
        summary_label: "AutoCAD",
        vendor: CounterpartVendor::Autodesk,
        adobe_products: &[],
        non_adobe_windows_exes: &[
            r"Autodesk\AutoCAD 2027\acad.exe",
            r"Autodesk\AutoCAD 2026\acad.exe",
            r"Autodesk\AutoCAD 2025\acad.exe",
        ],
        non_adobe_mac_bundles: &["Autodesk/AutoCAD 2027/AutoCAD 2027.app", "Autodesk/AutoCAD 2026/AutoCAD 2026.app"],
        search_terms: &["autocad", "autodesk", "dwg", "dxf"],
    },
    CounterpartSpec {
        app_id: "gridcraft",
        display_name: "Microsoft Excel",
        summary_label: "Microsoft Excel",
        vendor: CounterpartVendor::Microsoft,
        adobe_products: &[],
        non_adobe_windows_exes: &[r"Microsoft Office\root\Office16\EXCEL.EXE"],
        non_adobe_mac_bundles: &["Microsoft Excel.app"],
        search_terms: &["microsoft excel", "excel", "xlsx"],
    },
    CounterpartSpec {
        app_id: "texturecraft",
        display_name: "Adobe Substance 3D Painter",
        summary_label: "Adobe Substance 3D Painter (11.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Substance3DPainter],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe substance 3d painter", "substance 3d painter", "substance painter"],
    },
    CounterpartSpec {
        app_id: "materialcraft",
        display_name: "Adobe Substance 3D Designer",
        summary_label: "Adobe Substance 3D Designer (15.0)",
        vendor: CounterpartVendor::Adobe,
        adobe_products: &[AdobeProductId::Substance3DDesigner],
        non_adobe_windows_exes: &[],
        non_adobe_mac_bundles: &[],
        search_terms: &["adobe substance 3d designer", "substance 3d designer", "substance designer"],
    },
];

/// Iterator over the primary Adobe Creative Cloud 2027 & Acrobat DC 26.2 product specifications.
pub fn adobe_cc_2027_specs() -> impl Iterator<Item = &'static AdobeProductSpec> {
    ADOBE_PRODUCTS.iter().filter(|p| p.cc_2027_lineup)
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
        assert!(CounterpartSpec::by_app_id("filmcraft").unwrap().matches_query("media encoder 2027"));
        assert!(CounterpartSpec::by_app_id("pdfcraft").unwrap().matches_query("acrobat dc"));
        assert!(CounterpartSpec::by_app_id("anicraft").unwrap().matches_query("adobe animate"));
        assert!(CounterpartSpec::by_app_id("texturecraft").unwrap().matches_query("substance 3d painter"));
        assert!(CounterpartSpec::by_app_id("materialcraft").unwrap().matches_query("substance 3d designer"));
    }
}
