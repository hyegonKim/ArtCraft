//! Missing-Font Auto-Resolver (`resolve_missing_font`).
//!
//! Normalizes PostScript names, Adobe InDesign `$ID/...` identifiers, and weight/italic suffixed
//! font strings (such as `"HelveticaNeue-Bold"`, `"AdobeMyungjoStd-Medium"`, `"NotoSansCJKkr-Regular"`)
//! into the closest available catalog family, CSS/OpenType numeric weight, italic flag, and
//! [`FontClassification`] fallback (`Sans`, `Serif`, `Mono`, `Cjk`).

use crate::packs::FontClassification;
use crate::{FONTS, Font, LATIN_FONTS};

/// How a missing font name matched an available family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontMatchKind {
    /// Case/punctuation-insensitive exact match to an available family name.
    ExactFamily,
    /// Known PostScript or metric-compatible substitute (e.g. `HelveticaNeue` -> `Liberation Sans`).
    MetricSubstitute,
    /// Script-aware CJK substitute (e.g. Korean `AdobeMyungjoStd` / `NotoSansCJKkr` -> `NanumGothic`).
    ScriptSubstitute,
    /// Broad classification fallback (`Sans`, `Serif`, `Mono`, `Cjk`).
    ClassificationFallback,
}

/// Resolution result for a missing or foreign document font reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFont {
    /// Original raw font identifier requested by the document.
    pub requested: String,
    /// Cleaned family stem after stripping `$ID/`, vendor prefixes, and weight/style suffixes.
    pub normalized_stem: String,
    /// Resolved family name chosen from the available catalog.
    pub family: String,
    /// Best matching style label (e.g. `"Regular"`, `"Bold"`, `"Italic"`, `"Bold Italic"`, `"SemiBold"`).
    pub style: String,
    /// Inferred OpenType/CSS weight class (`100..=900`, `400` = Regular, `700` = Bold).
    pub weight: u16,
    /// Whether an italic or oblique posture was requested.
    pub italic: bool,
    /// Inferred typographic classification (`Sans`, `Serif`, `Mono`, `Cjk`).
    pub classification: FontClassification,
    /// How the family match was chosen.
    pub match_kind: FontMatchKind,
    /// Exact static/variable [`Font`] entry from [`FONTS`] or [`LATIN_FONTS`], when matched against the built-in catalog.
    pub catalog_font: Option<&'static Font>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CjkSubscript {
    Korean,
    JapaneseSerif,
    JapaneseSans,
    Chinese,
}

/// Resolve a missing PostScript / `$ID/` / weight-suffixed font name against all bundled Craft catalogs
/// ([`FONTS`] and [`LATIN_FONTS`]).
pub fn resolve_missing_font(requested: &str) -> ResolvedFont {
    let mut families: Vec<&str> = Vec::new();
    for f in FONTS.iter().chain(LATIN_FONTS.iter()) {
        if !families.contains(&f.family) {
            families.push(f.family);
        }
    }
    let mut resolved = resolve_missing_font_with_catalog(requested, &families);
    if let Some((face, actual_style)) =
        pick_best_catalog_face(&resolved.family, resolved.weight, resolved.italic)
    {
        resolved.style = actual_style.to_string();
        resolved.catalog_font = Some(face);
    }
    resolved
}

/// Resolve a missing font name against a caller-supplied list of available family names.
pub fn resolve_missing_font_with_catalog(
    requested: &str,
    available_families: &[&str],
) -> ResolvedFont {
    let cleaned = strip_adobe_prefixes(requested);
    let (stem_raw, weight, italic) = split_family_and_style(&cleaned);
    let normalized_stem = humanize_camel_stem(&stem_raw);
    let key = compact_alnum_lower(&normalized_stem);

    let cjk_sub = detect_cjk_subscript(&key);
    let classification = if cjk_sub.is_some() {
        FontClassification::Cjk
    } else if is_monospace_key(&key) {
        FontClassification::Mono
    } else if is_serif_key(&key) {
        FontClassification::Serif
    } else {
        FontClassification::Sans
    };

    let desired_style = canonical_style_label(weight, italic);

    // 1. Direct family match against available_families
    if let Some(exact) = available_families
        .iter()
        .copied()
        .find(|fam| compact_alnum_lower(fam) == key && !key.is_empty())
    {
        return ResolvedFont {
            requested: requested.to_string(),
            normalized_stem,
            family: exact.to_string(),
            style: desired_style.to_string(),
            weight,
            italic,
            classification,
            match_kind: FontMatchKind::ExactFamily,
            catalog_font: None,
        };
    }

    // 2. Script-specific CJK resolution
    if let Some(sub) = cjk_sub {
        let candidates: &[&str] = match sub {
            CjkSubscript::Korean => &["NanumGothic", "Noto Sans CJK SC", "BIZ UDPGothic"],
            CjkSubscript::JapaneseSerif => &[
                "Shippori Mincho",
                "BIZ UDMincho",
                "BIZ UDPGothic",
                "Noto Sans CJK SC",
            ],
            CjkSubscript::JapaneseSans => &["BIZ UDPGothic", "Shippori Mincho", "Noto Sans CJK SC"],
            CjkSubscript::Chinese => &[
                "Noto Sans CJK SC",
                "LXGW WenKai Lite",
                "Shippori Mincho",
                "BIZ UDPGothic",
            ],
        };
        if let Some(chosen) = first_available(candidates, available_families) {
            return ResolvedFont {
                requested: requested.to_string(),
                normalized_stem,
                family: chosen.to_string(),
                style: desired_style.to_string(),
                weight,
                italic,
                classification: FontClassification::Cjk,
                match_kind: FontMatchKind::ScriptSubstitute,
                catalog_font: None,
            };
        }
    }

    // 3. Known PostScript / metric-compatible family aliases
    if let Some(alias_candidates) = metric_alias_candidates(&key)
        && let Some(chosen) = first_available(alias_candidates, available_families)
    {
        return ResolvedFont {
            requested: requested.to_string(),
            normalized_stem,
            family: chosen.to_string(),
            style: desired_style.to_string(),
            weight,
            italic,
            classification,
            match_kind: FontMatchKind::MetricSubstitute,
            catalog_font: None,
        };
    }

    // 4. Classification fallback
    let fallback_candidates: &[&str] = match classification {
        FontClassification::Sans => &[
            "Inter",
            "Liberation Sans",
            "Source Sans 3",
            "Roboto",
            "Open Sans",
            "BIZ UDPGothic",
        ],
        FontClassification::Serif => &[
            "Source Serif 4",
            "Liberation Serif",
            "Merriweather",
            "Caladea",
            "Playfair Display",
            "Shippori Mincho",
        ],
        FontClassification::Mono => &["Liberation Mono", "Inter", "Roboto"],
        FontClassification::Cjk => &[
            "Noto Sans CJK SC",
            "BIZ UDPGothic",
            "Shippori Mincho",
            "NanumGothic",
        ],
    };

    let family = first_available(fallback_candidates, available_families)
        .or_else(|| available_families.first().copied())
        .unwrap_or(match classification {
            FontClassification::Sans => "Inter",
            FontClassification::Serif => "Source Serif 4",
            FontClassification::Mono => "Liberation Mono",
            FontClassification::Cjk => "Noto Sans CJK SC",
        })
        .to_string();

    ResolvedFont {
        requested: requested.to_string(),
        normalized_stem,
        family,
        style: desired_style.to_string(),
        weight,
        italic,
        classification,
        match_kind: FontMatchKind::ClassificationFallback,
        catalog_font: None,
    }
}

fn first_available<'a>(candidates: &[&str], available: &[&'a str]) -> Option<&'a str> {
    for wanted in candidates {
        let wanted_key = compact_alnum_lower(wanted);
        if let Some(&found) = available
            .iter()
            .find(|&&av| compact_alnum_lower(av) == wanted_key)
        {
            return Some(found);
        }
    }
    None
}

fn strip_adobe_prefixes(raw: &str) -> String {
    let mut s = raw.trim();
    while let Some(rest) = s.strip_prefix("$ID/") {
        s = rest.trim();
    }
    // Strip subset 6-uppercase-letter PDF font tags like "ABCDEF+HelveticaNeue-Bold"
    if let Some((prefix, rest)) = s.split_once('+')
        && prefix.len() == 6
        && prefix.bytes().all(|b| b.is_ascii_uppercase())
        && !rest.is_empty()
    {
        s = rest.trim();
    }
    // InDesign tab-delimited Family\tStyle -> Family-Style
    s.replace('\t', "-")
}

fn split_family_and_style(cleaned: &str) -> (String, u16, bool) {
    // Split on the last '-' or ',' if present; also inspect trailing space-separated style words.
    let (base_candidate, raw_suffix) = if let Some((lhs, rhs)) = cleaned.rsplit_once('-') {
        // Keep CJK hyphenated bases like "MS-PGothic" or "MS-Mincho" intact when rhs is a family noun!
        if is_style_descriptor(rhs) {
            (lhs.trim(), Some(rhs.trim()))
        } else {
            (cleaned, None)
        }
    } else if let Some((lhs, rhs)) = cleaned.rsplit_once(',') {
        if is_style_descriptor(rhs) {
            (lhs.trim(), Some(rhs.trim()))
        } else {
            (cleaned, None)
        }
    } else {
        (cleaned, None)
    };

    if let Some(suffix) = raw_suffix {
        let (w, it) = parse_style_tokens(suffix);
        return (strip_ps_vendor_tail(base_candidate), w, it);
    }

    // Check trailing whitespace words such as "Helvetica Neue Bold Italic"
    let words: Vec<&str> = base_candidate.split_whitespace().collect();
    let mut cut = words.len();
    let mut weight = 400u16;
    let mut italic = false;
    let mut matched_any = false;
    while cut > 1 {
        let word = words[cut - 1];
        if is_style_descriptor(word) {
            let (w, it) = parse_style_tokens(word);
            if w != 400 {
                weight = w;
            }
            if it {
                italic = true;
            }
            matched_any = true;
            cut -= 1;
        } else {
            break;
        }
    }

    if matched_any {
        let joined = words[..cut].join(" ");
        (strip_ps_vendor_tail(&joined), weight, italic)
    } else {
        (strip_ps_vendor_tail(base_candidate), 400, false)
    }
}

fn strip_ps_vendor_tail(stem: &str) -> String {
    let trimmed = stem.trim();
    for suffix in ["PSMT", "MT"] {
        if trimmed.len() > suffix.len() + 3
            && trimmed.ends_with(suffix)
            && let Some(stripped) = trimmed.strip_suffix(suffix)
        {
            return stripped.trim_end_matches('-').to_string();
        }
    }
    trimmed.to_string()
}

fn is_style_descriptor(token: &str) -> bool {
    let mut s = token.trim().to_ascii_lowercase();
    for noise in ["psmt", "mt", " variable", "variable"] {
        s = s.replace(noise, "");
    }
    let s = s.replace(['_', ' '], "");
    if s.is_empty() {
        return false;
    }
    let known = [
        "thin",
        "hairline",
        "extralight",
        "ultralight",
        "light",
        "demilight",
        "regular",
        "roman",
        "book",
        "normal",
        "text",
        "medium",
        "semibold",
        "demibold",
        "demi",
        "bold",
        "extrabold",
        "ultrabold",
        "heavy",
        "black",
        "ultra",
        "italic",
        "it",
        "ita",
        "oblique",
        "obl",
        "slanted",
        "inclined",
        "condensed",
        "cond",
        "cn",
        "narrow",
        "expanded",
        "extended",
    ];
    if known.contains(&s.as_str()) {
        return true;
    }
    // Compound PostScript suffixes like "BoldItalic", "SemiboldIt", "BoldOblique", "LightIt", "MediumCondIt", "W3", "W6"
    if matches!(s.as_bytes(), [b'w', b'1'..=b'9']) {
        return true;
    }
    let mut rest = s.as_str();
    for piece in [
        "extrabold",
        "ultrabold",
        "extralight",
        "ultralight",
        "semibold",
        "demibold",
        "demilight",
        "hairline",
        "condensed",
        "narrow",
        "expanded",
        "extended",
        "regular",
        "medium",
        "normal",
        "italic",
        "oblique",
        "slanted",
        "roman",
        "light",
        "heavy",
        "black",
        "ultra",
        "thin",
        "book",
        "demi",
        "bold",
        "cond",
        "obl",
        "ita",
        "it",
        "cn",
    ] {
        rest = rest.trim_start_matches(piece).trim_end_matches(piece);
    }
    rest.is_empty()
}

fn parse_style_tokens(raw_style: &str) -> (u16, bool) {
    let s = raw_style
        .to_ascii_lowercase()
        .replace("psmt", "")
        .replace("mt", "");

    let italic = s.contains("italic")
        || s.contains("oblique")
        || s.contains("slanted")
        || s.ends_with("it")
        || s == "it"
        || s.ends_with("obl");

    // Hiragino / Japanese W1..W9 weight notation
    let weight = if s.contains("extrabold")
        || s.contains("ultrabold")
        || s.contains("heavy")
        || s.contains("w8")
    {
        800
    } else if s.contains("black")
        || s.contains("ultra") && !s.contains("ultralight")
        || s.contains("w9")
    {
        900
    } else if s.contains("semibold")
        || s.contains("demibold")
        || s.contains("demi") && !s.contains("demilight")
        || s.contains("w6")
    {
        600
    } else if s.contains("bold") || s.contains("w7") {
        700
    } else if s.contains("medium") || s.contains("w5") {
        500
    } else if s.contains("extralight") || s.contains("ultralight") || s.contains("w2") {
        200
    } else if s.contains("light") || s.contains("demilight") || s.contains("w3") {
        300
    } else if s.contains("thin") || s.contains("hairline") || s.contains("w1") {
        100
    } else {
        400
    };

    (weight, italic)
}

fn canonical_style_label(weight: u16, italic: bool) -> &'static str {
    match (weight, italic) {
        (600, false) => "SemiBold",
        (600, true) => "Bold Italic",
        (w, true) if w >= 700 => "Bold Italic",
        (w, false) if w >= 700 => "Bold",
        (_, true) => "Italic",
        _ => "Regular",
    }
}

fn humanize_camel_stem(stem: &str) -> String {
    let chars: Vec<char> = stem.chars().collect();
    let mut out = String::with_capacity(chars.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase()
                || (prev.is_ascii_uppercase() && next_lower && !out.ends_with(' '))
            {
                out.push(' ');
            }
        } else if i > 0
            && c.is_ascii_digit()
            && chars[i - 1].is_ascii_alphabetic()
            && !out.ends_with(' ')
        {
            out.push(' ');
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn detect_cjk_subscript(key: &str) -> Option<CjkSubscript> {
    // Korean Hangul / Myungjo / Gothic cues
    for kw in [
        "myungjo",
        "batang",
        "dotum",
        "gulim",
        "malgun",
        "nanum",
        "applesdgothic",
        "notosanscjkkr",
        "notoserifcjkkr",
        "notosanskr",
        "notoserifkr",
        "sourcehansanskr",
        "sourcehanserifkr",
        "sourcehansansk",
        "hangul",
    ] {
        if key.contains(kw) {
            return Some(CjkSubscript::Korean);
        }
    }

    // Japanese Mincho (serif) cues
    for kw in [
        "mincho",
        "kozmin",
        "hiraginomincho",
        "hiramin",
        "ryumin",
        "shippori",
        "notoserifcjkjp",
        "notoserifjp",
        "sourcehanserifjp",
    ] {
        if key.contains(kw) {
            return Some(CjkSubscript::JapaneseSerif);
        }
    }

    // Japanese Gothic (sans) cues
    for kw in [
        "kozgo",
        "pgothic",
        "msgothic",
        "yugothic",
        "hiraginokaku",
        "hiraginomaru",
        "hirakaku",
        "meiryo",
        "bizud",
        "notosanscjkjp",
        "notosansjp",
        "sourcehansansjp",
    ] {
        if key.contains(kw) {
            return Some(CjkSubscript::JapaneseSans);
        }
    }

    // Chinese Song / Ming / Heiti / Kaiti / FangSong / YaHei / PingFang / CJK SC/TC/HK
    for kw in [
        "adobesong",
        "adobeming",
        "adobeheiti",
        "adobekaiti",
        "adobefangsong",
        "simsun",
        "nsimsun",
        "simhei",
        "yahei",
        "pingfang",
        "stheit",
        "stsong",
        "stkaiti",
        "wenkai",
        "notosanscjk",
        "notoserifcjk",
        "sourcehansans",
        "sourcehanserif",
    ] {
        if key.contains(kw) {
            return Some(CjkSubscript::Chinese);
        }
    }

    None
}

fn is_monospace_key(key: &str) -> bool {
    [
        "mono",
        "courier",
        "consolas",
        "menlo",
        "monaco",
        "firacode",
        "jetbrains",
        "inconsolata",
        "sourcecode",
        "cascadia",
        "lucida",
        "fixedsys",
        "terminal",
    ]
    .iter()
    .any(|kw| key.contains(kw))
}

fn is_serif_key(key: &str) -> bool {
    [
        "serif",
        "times",
        "minion",
        "garamond",
        "baskerville",
        "georgia",
        "cambria",
        "palatino",
        "bookantiqua",
        "caslon",
        "bodoni",
        "didot",
        "sabon",
        "bembo",
        "charter",
        "century",
        "merriweather",
        "playfair",
        "caladea",
    ]
    .iter()
    .any(|kw| key.contains(kw))
}

fn metric_alias_candidates(key: &str) -> Option<&'static [&'static str]> {
    if key.contains("helvetica")
        || key.contains("arial")
        || key.contains("nimbus")
        || key.contains("swiss")
    {
        Some(&["Liberation Sans", "Inter", "Roboto"])
    } else if key.contains("calibri") {
        Some(&["Carlito", "Source Sans 3", "Open Sans"])
    } else if key.contains("cambria") {
        Some(&["Caladea", "Source Serif 4", "Liberation Serif"])
    } else if key.contains("times") {
        Some(&["Liberation Serif", "Source Serif 4", "Merriweather"])
    } else if key.contains("courier") || key.contains("consolas") || key.contains("menlo") {
        Some(&["Liberation Mono"])
    } else if key.contains("myriad") || key.contains("frutiger") || key.contains("segoe") {
        Some(&["Source Sans 3", "Open Sans", "Inter"])
    } else if key.contains("minion")
        || key.contains("garamond")
        || key.contains("caslon")
        || key.contains("baskerville")
    {
        Some(&["Source Serif 4", "Merriweather", "Liberation Serif"])
    } else if key.contains("futura")
        || key.contains("avenir")
        || key.contains("proxima")
        || key.contains("gotham")
    {
        Some(&["Montserrat", "Poppins", "Inter"])
    } else if key.contains("bodoni") || key.contains("didot") {
        Some(&["Playfair Display", "Source Serif 4"])
    } else {
        None
    }
}

fn pick_best_catalog_face(
    family: &str,
    weight: u16,
    italic: bool,
) -> Option<(&'static Font, &'static str)> {
    let faces: Vec<&'static Font> = FONTS
        .iter()
        .chain(LATIN_FONTS.iter())
        .filter(|f| f.family == family)
        .collect();
    if faces.is_empty() {
        return None;
    }

    let mut best_face = faces[0];
    let mut best_score = i32::MAX;
    for face in faces {
        let face_italic = face.style.contains("Italic");
        let italic_penalty = if face_italic == italic { 0 } else { 1000 };
        let weight_penalty = if face.style.ends_with("Variable") {
            // Variable faces cover the continuous weight axis natively (small penalty vs exact static style).
            15
        } else {
            let face_weight = match face.style {
                "SemiBold" | "Semibold" => 600i32,
                "Bold" | "Bold Italic" => 700,
                _ => 400,
            };
            (face_weight - i32::from(weight)).abs()
        };
        let score = italic_penalty + weight_penalty;
        if score < best_score {
            best_score = score;
            best_face = face;
        }
    }
    Some((best_face, best_face.style))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_postscript_adobe_id_and_cjk_names() {
        let helv = resolve_missing_font("HelveticaNeue-Bold");
        assert_eq!(helv.family, "Liberation Sans");
        assert_eq!(helv.style, "Bold");
        assert_eq!(helv.weight, 700);
        assert!(!helv.italic);
        assert_eq!(helv.classification, FontClassification::Sans);
        assert_eq!(helv.match_kind, FontMatchKind::MetricSubstitute);
        assert!(helv.catalog_font.is_some());

        let indesign_id = resolve_missing_font("$ID/MinionPro-BoldIt");
        assert_eq!(indesign_id.family, "Source Serif 4");
        assert_eq!(indesign_id.weight, 700);
        assert!(indesign_id.italic);
        assert_eq!(indesign_id.classification, FontClassification::Serif);

        let myungjo = resolve_missing_font("AdobeMyungjoStd-Medium");
        assert_eq!(myungjo.family, "NanumGothic");
        assert_eq!(myungjo.weight, 500);
        assert!(!myungjo.italic);
        assert_eq!(myungjo.classification, FontClassification::Cjk);
        assert_eq!(myungjo.match_kind, FontMatchKind::ScriptSubstitute);

        let kr = resolve_missing_font("NotoSansCJKkr-Regular");
        assert_eq!(kr.family, "NanumGothic");
        assert_eq!(kr.weight, 400);
        assert_eq!(kr.classification, FontClassification::Cjk);

        let jp = resolve_missing_font("$ID/KozMinPr6N-Regular");
        assert_eq!(jp.family, "Shippori Mincho");
        assert_eq!(jp.classification, FontClassification::Cjk);

        let mono = resolve_missing_font("ABCDEF+CourierNewPS-BoldItalicMT");
        assert_eq!(mono.family, "Liberation Mono");
        assert_eq!(mono.style, "Bold Italic");
        assert_eq!(mono.weight, 700);
        assert!(mono.italic);
        assert_eq!(mono.classification, FontClassification::Mono);
    }

    #[test]
    fn resolves_exact_catalog_families_and_custom_catalogs() {
        let exact = resolve_missing_font("SourceSans3-Semibold");
        assert_eq!(exact.family, "Source Sans 3");
        assert_eq!(exact.style, "Semibold");
        assert_eq!(exact.weight, 600);
        assert_eq!(exact.match_kind, FontMatchKind::ExactFamily);

        let custom = resolve_missing_font_with_catalog(
            "UnknownSlabSerif-LightOblique",
            &["Caladea", "Inter"],
        );
        assert_eq!(custom.family, "Caladea");
        assert_eq!(custom.weight, 300);
        assert!(custom.italic);
        assert_eq!(custom.classification, FontClassification::Serif);
        assert_eq!(custom.match_kind, FontMatchKind::ClassificationFallback);
    }
}
