//! Creative Cloud-style Curated Font Packs (`FontPackCategory`) mapping bundled and Latin catalog
//! families with typographic specimen metadata.

use crate::{FONTS, Font, LATIN_FONTS};

/// Broad typographic classification used by font packs and missing-font fallback resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontClassification {
    /// Grotesque, neo-grotesque, humanist, and geometric sans-serif faces.
    Sans,
    /// Editorial, transitional, old-style, and high-contrast display serif faces.
    Serif,
    /// Fixed-width code and tabular monospace faces.
    Mono,
    /// East Asian Chinese, Japanese, and Korean faces (`Hans`, `Hant`, `Jpan`, `Kore`, `Hang`).
    Cjk,
}

impl FontClassification {
    /// Canonical lowercase identifier (`"sans"`, `"serif"`, `"mono"`, `"cjk"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sans => "sans",
            Self::Serif => "serif",
            Self::Mono => "mono",
            Self::Cjk => "cjk",
        }
    }
}

/// Curated Creative Cloud-equivalent font pack categories for Craft Suite workflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontPackCategory {
    /// Long-form books, magazines, newspapers, and academic layout (`DesignCraft`, `WordCraft`, `PdfCraft`).
    EditorialPublishing,
    /// Lower-thirds, title cards, end credits, and broadcast supers (`FilmCraft`, `EffectCraft`).
    CinemaAndBroadcast,
    /// High-legibility interface systems, design tokens, and developer tooling (`FigCraft`, `ArtCraft`).
    UiAndWebDesign,
    /// Pan-CJK Simplified Chinese, Japanese Mincho/UD Gothic, and Korean Hangul typography.
    GlobalCjkTypography,
}

impl FontPackCategory {
    /// All curated font pack categories in display order.
    pub const ALL: [Self; 4] = [
        Self::EditorialPublishing,
        Self::CinemaAndBroadcast,
        Self::UiAndWebDesign,
        Self::GlobalCjkTypography,
    ];

    /// Stable slug identifier for persistence and activation commands.
    pub fn slug(self) -> &'static str {
        match self {
            Self::EditorialPublishing => "editorial-publishing",
            Self::CinemaAndBroadcast => "cinema-and-broadcast",
            Self::UiAndWebDesign => "ui-and-web-design",
            Self::GlobalCjkTypography => "global-cjk-typography",
        }
    }
}

/// Specimen metadata for one curated font family inside a [`FontPack`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontSpecimen {
    /// Exact `Font::family` name in `FONTS` or `LATIN_FONTS`.
    pub family: &'static str,
    /// Typographic classification.
    pub classification: FontClassification,
    /// Recommended point-size range `(min_pt, max_pt)`.
    pub recommended_point_range: (u16, u16),
    /// Whether the catalog ships variable (`fvar`) faces for this family.
    pub is_variable: bool,
    /// ISO 15924 scripts emphasized by this specimen.
    pub primary_scripts: &'static [&'static str],
    /// Display headline specimen string.
    pub sample_headline: &'static str,
    /// Paragraph body specimen string.
    pub sample_body: &'static str,
}

impl FontSpecimen {
    /// Resolve all matching [`Font`] face entries across [`FONTS`] and [`LATIN_FONTS`].
    pub fn catalog_faces(&self) -> Vec<&'static Font> {
        FONTS
            .iter()
            .chain(LATIN_FONTS.iter())
            .filter(|f| f.family == self.family)
            .collect()
    }
}

/// A curated font pack grouping catalog families for a creative discipline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontPack {
    /// Category enum tag.
    pub category: FontPackCategory,
    /// Pack identifier (e.g. `"pack.editorial-publishing"`).
    pub id: &'static str,
    /// Human-readable pack title.
    pub title: &'static str,
    /// Curatorial summary describing intended creative use.
    pub description: &'static str,
    /// Curated family specimens included in this pack.
    pub specimens: &'static [FontSpecimen],
}

impl FontPack {
    /// Resolve every [`Font`] file belonging to any family in this pack.
    pub fn resolved_faces(&self) -> Vec<&'static Font> {
        let mut out = Vec::new();
        for specimen in self.specimens {
            out.extend(specimen.catalog_faces());
        }
        out
    }
}

static EDITORIAL_SPECIMENS: &[FontSpecimen] = &[
    FontSpecimen {
        family: "Source Serif 4",
        classification: FontClassification::Serif,
        recommended_point_range: (9, 36),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "The Craft of Long-Form Typography",
        sample_body: "Balanced proportions and crisp wedge serifs sustain effortless reading across dense editorial columns.",
    },
    FontSpecimen {
        family: "Playfair Display",
        classification: FontClassification::Serif,
        recommended_point_range: (18, 96),
        is_variable: true,
        primary_scripts: &["Latn"],
        sample_headline: "Autumn Issue — Feature Essay",
        sample_body: "High-contrast hairlines and delicate ball terminals bring classic magazine authority to mastheads.",
    },
    FontSpecimen {
        family: "Merriweather",
        classification: FontClassification::Serif,
        recommended_point_range: (9, 28),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Chapter IV: Light & Silver Halide",
        sample_body: "Generous x-height and sturdy serifs preserve clarity in both print proofs and high-DPI tablet editions.",
    },
    FontSpecimen {
        family: "Liberation Serif",
        classification: FontClassification::Serif,
        recommended_point_range: (9, 24),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Quarterly Monograph & Citations",
        sample_body: "Metric-compatible proportions ensure pagination stability when exchanging manuscripts across platforms.",
    },
    FontSpecimen {
        family: "Caladea",
        classification: FontClassification::Serif,
        recommended_point_range: (10, 30),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: " Editorial Review & Book Proofs",
        sample_body: "Warm humanist rhythm tailored for contemporary reports, essays, and structured publishing templates.",
    },
];

static CINEMA_SPECIMENS: &[FontSpecimen] = &[
    FontSpecimen {
        family: "Montserrat",
        classification: FontClassification::Sans,
        recommended_point_range: (14, 120),
        is_variable: true,
        primary_scripts: &["Latn"],
        sample_headline: "DIRECTED BY ELENA VANCE",
        sample_body: "Geometric uppercase tracking holds razor-sharp edges over anamorphic film grain and HDR title cards.",
    },
    FontSpecimen {
        family: "Poppins",
        classification: FontClassification::Sans,
        recommended_point_range: (12, 84),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "LIVE BROADCAST • STUDIO A",
        sample_body: "Monolinear circular counters read cleanly at a glance on lower-thirds, chyrons, and motion graphics.",
    },
    FontSpecimen {
        family: "Lato",
        classification: FontClassification::Sans,
        recommended_point_range: (11, 48),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Original Score & Sound Design",
        sample_body: "Semi-rounded details lend approachable warmth to documentary subtitles and rolling end-credit crawls.",
    },
    FontSpecimen {
        family: "Carlito",
        classification: FontClassification::Sans,
        recommended_point_range: (10, 36),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Episode 04 — Dialogue Spotting List",
        sample_body: "Compact horizontal metrics fit timecoded broadcast scripts and multi-language closed-caption safe zones.",
    },
];

static UI_WEB_SPECIMENS: &[FontSpecimen] = &[
    FontSpecimen {
        family: "Inter",
        classification: FontClassification::Sans,
        recommended_point_range: (10, 40),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Design System Tokens & Components",
        sample_body: "Crafted specifically for computer screens with a tall x-height and unambiguous glyph distinction.",
    },
    FontSpecimen {
        family: "Roboto",
        classification: FontClassification::Sans,
        recommended_point_range: (10, 48),
        is_variable: true,
        primary_scripts: &["Latn"],
        sample_headline: "Responsive Layout Inspector 1440px",
        sample_body: "Continuous weight and width axes adapt seamlessly across dense toolbars, mobile cards, and dashboards.",
    },
    FontSpecimen {
        family: "Source Sans 3",
        classification: FontClassification::Sans,
        recommended_point_range: (10, 36),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "Inspector Properties & Layer Tree",
        sample_body: "Clean, open counters keep micro-labels legible inside dark-mode creative application panels.",
    },
    FontSpecimen {
        family: "Open Sans",
        classification: FontClassification::Sans,
        recommended_point_range: (10, 42),
        is_variable: true,
        primary_scripts: &["Latn"],
        sample_headline: "Accessible Web Portal Typography",
        sample_body: "Upright stress and neutral forms deliver dependable readability across desktop and mobile browsers.",
    },
    FontSpecimen {
        family: "Nunito Sans",
        classification: FontClassification::Sans,
        recommended_point_range: (11, 44),
        is_variable: true,
        primary_scripts: &["Latn"],
        sample_headline: "Interactive Prototype Walkthrough",
        sample_body: "Optical size and width variations balance friendly onboarding headers with crisp data tables.",
    },
    FontSpecimen {
        family: "Liberation Mono",
        classification: FontClassification::Mono,
        recommended_point_range: (9, 20),
        is_variable: false,
        primary_scripts: &["Latn"],
        sample_headline: "0x00FF_A8C0 => blend_srgb_linear(1.0)",
        sample_body: "Zero, capital O, numeral 1, and lowercase l remain unmistakable in shader editors and hex inspectors.",
    },
];

static GLOBAL_CJK_SPECIMENS: &[FontSpecimen] = &[
    FontSpecimen {
        family: "Noto Sans CJK SC",
        classification: FontClassification::Cjk,
        recommended_point_range: (10, 64),
        is_variable: false,
        primary_scripts: &["Hans", "Latn"],
        sample_headline: "现代中文字体排印与视觉设计",
        sample_body: "均匀的字面率与清晰的笔画结构，适用于界面标题、演示文稿与多语种混排正文。",
    },
    FontSpecimen {
        family: "Shippori Mincho",
        classification: FontClassification::Cjk,
        recommended_point_range: (10, 72),
        is_variable: false,
        primary_scripts: &["Jpan", "Latn"],
        sample_headline: "静謐な明朝体と縦組みの美しさ",
        sample_body: "伝統的な築地体を受け継ぐ端正なウロコと縦書き字形が、文芸書や見出しに品格を与えます。",
    },
    FontSpecimen {
        family: "BIZ UDPGothic",
        classification: FontClassification::Cjk,
        recommended_point_range: (9, 48),
        is_variable: false,
        primary_scripts: &["Jpan", "Latn"],
        sample_headline: "ユニバーサルデザインゴシック表示",
        sample_body: "濁点・半濁点の識別性と広いフトコロにより、情報画面やビジネス文書で高い視認性を発揮します。",
    },
    FontSpecimen {
        family: "BIZ UDMincho",
        classification: FontClassification::Cjk,
        recommended_point_range: (10, 48),
        is_variable: false,
        primary_scripts: &["Jpan", "Latn"],
        sample_headline: "読みやすさと格調を両立した明朝",
        sample_body: "横画の太さを確保したユニバーサルデザイン設計で、長文レポートや公的文書に最適です。",
    },
    FontSpecimen {
        family: "NanumGothic",
        classification: FontClassification::Cjk,
        recommended_point_range: (10, 56),
        is_variable: false,
        primary_scripts: &["Hang", "Kore"],
        sample_headline: "한글 타이포그래피와 디지털 인터페이스",
        sample_body: "부드러운 곡선과 선명한 자소 균형으로 현대 한글 문서와 UI 화면에서 뛰어난 가독성을 제공합니다.",
    },
];

static FONT_PACKS: [FontPack; 4] = [
    FontPack {
        category: FontPackCategory::EditorialPublishing,
        id: "pack.editorial-publishing",
        title: "Editorial & Book Publishing",
        description: "Text and high-contrast display serifs curated for multi-column layouts, books, and magazines.",
        specimens: EDITORIAL_SPECIMENS,
    },
    FontPack {
        category: FontPackCategory::CinemaAndBroadcast,
        id: "pack.cinema-and-broadcast",
        title: "Cinema Titles & Broadcast Graphics",
        description: "Geometric display faces and caption-safe sans-serifs engineered for motion titles and chyrons.",
        specimens: CINEMA_SPECIMENS,
    },
    FontPack {
        category: FontPackCategory::UiAndWebDesign,
        id: "pack.ui-and-web-design",
        title: "UI Systems & Web Typography",
        description: "Screen-optimized grotesque, humanist variable sans, and developer monospace families.",
        specimens: UI_WEB_SPECIMENS,
    },
    FontPack {
        category: FontPackCategory::GlobalCjkTypography,
        id: "pack.global-cjk-typography",
        title: "Global CJK Typography",
        description: "Simplified Chinese, Japanese Mincho & UD Gothic, and Korean Hangul families with vertical support.",
        specimens: GLOBAL_CJK_SPECIMENS,
    },
];

/// Retrieve the curated [`FontPack`] for `category`.
pub fn font_pack(category: FontPackCategory) -> &'static FontPack {
    match category {
        FontPackCategory::EditorialPublishing => &FONT_PACKS[0],
        FontPackCategory::CinemaAndBroadcast => &FONT_PACKS[1],
        FontPackCategory::UiAndWebDesign => &FONT_PACKS[2],
        FontPackCategory::GlobalCjkTypography => &FONT_PACKS[3],
    }
}

/// Return all four curated [`FontPack`] definitions.
pub fn all_font_packs() -> &'static [FontPack] {
    &FONT_PACKS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curated_pack_maps_to_real_bundled_families() {
        assert_eq!(all_font_packs().len(), 4);
        for category in FontPackCategory::ALL {
            let pack = font_pack(category);
            assert_eq!(pack.category, category);
            assert!(!pack.specimens.is_empty(), "{} has no specimens", pack.id);
            for specimen in pack.specimens {
                let faces = specimen.catalog_faces();
                assert!(
                    !faces.is_empty(),
                    "pack {} references unknown catalog family {:?}",
                    pack.id,
                    specimen.family
                );
                let has_var_face = faces.iter().any(|f| f.style.ends_with("Variable"));
                assert_eq!(
                    specimen.is_variable, has_var_face,
                    "specimen {} is_variable disagrees with manifest styles",
                    specimen.family
                );
            }
            assert!(!pack.resolved_faces().is_empty());
        }
    }
}
