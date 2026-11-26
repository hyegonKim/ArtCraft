//! Live preflight (the Preflight panel and the status-bar indicator): overset text, missing
//! fonts and glyphs, missing/modified or low-resolution graphics, bleed/trim safety, trim safe
//! zone, RGB/spot/registration ink checks, non-proportional scaling, minimum stroke weight, live
//! transparency, empty text frames, white or [Paper] set to overprint, and `.idpp` profiles.

use designcraft_color::{BlendMode, Color, ColorType, SwatchValue, swatch};
use designcraft_compose::{Cache, ComposedStory};
use designcraft_doc::{Content, Document, Intent, Item, SpreadRef};
use designcraft_fonts::FaceRef;
use designcraft_geom::Affine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, has_doc, str_param};
use crate::{EngineError, Result, Session};

mod white_overprint;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// `error` or `warning`.
    pub severity: &'static str,
    /// `LINKS`, `COLOR`, `IMAGES and OBJECTS`, `TEXT`, or `DOCUMENT`.
    #[serde(skip_serializing)]
    pub category: &'static str,
    pub kind: &'static str,
    pub message: String,
    /// Actionable Info/Fix guidance shown in the Preflight panel's Info drawer.
    #[serde(skip_serializing)]
    pub fix: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
}

/// Configurable Preflight Profile (`.idpp` compatible).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PreflightProfile {
    pub name: String,
    pub description: String,
    pub check_links: bool,
    pub check_modified_links: bool,
    pub check_overset: bool,
    pub check_missing_fonts: bool,
    pub check_missing_glyphs: bool,
    pub check_empty_text: bool,
    pub min_ppi: f64,
    pub check_bleed: bool,
    /// Minimum distance from page trim edge for non-bleeding objects (0 = disabled).
    pub safe_margin_pt: f64,
    /// Flag RGB swatches/objects (`false` = follow document Print intent; `true` = always flag).
    pub disallow_rgb: bool,
    /// Treat RGB usage as an error rather than a warning (e.g. PDF/X-1a, strict CMYK press).
    pub rgb_is_error: bool,
    pub disallow_spot: bool,
    pub check_registration: bool,
    pub check_white_overprint: bool,
    pub check_non_proportional_scaling: bool,
    /// Minimum stroke weight in points (`0.0` = disabled, e.g. `0.25` for hairline check).
    pub min_stroke_pt: f64,
    pub disallow_transparency: bool,
}

impl Default for PreflightProfile {
    fn default() -> Self {
        Self::basic()
    }
}

impl PreflightProfile {
    /// InDesign's default `[Basic]` profile (working profile).
    pub fn basic() -> Self {
        Self {
            name: "[Basic]".into(),
            description: "Default live preflight checks for links, text, fonts, resolution, and bleed.".into(),
            check_links: true,
            check_modified_links: true,
            check_overset: true,
            check_missing_fonts: true,
            check_missing_glyphs: true,
            check_empty_text: true,
            min_ppi: 150.0,
            check_bleed: true,
            safe_margin_pt: 0.0,
            disallow_rgb: false,
            rgb_is_error: false,
            disallow_spot: false,
            check_registration: false,
            check_white_overprint: true,
            check_non_proportional_scaling: false,
            min_stroke_pt: 0.0,
            disallow_transparency: false,
        }
    }

    pub fn digital_publishing() -> Self {
        Self {
            name: "Digital Publishing".into(),
            description: "RGB/EPUB/HTML screen publishing checks (72 ppi minimum, RGB allowed).".into(),
            min_ppi: 72.0,
            check_bleed: false,
            check_white_overprint: false,
            ..Self::basic()
        }
    }

    pub fn press_quality_cmyk() -> Self {
        Self {
            name: "Press Quality (CMYK)".into(),
            description: "Commercial sheetfed offset checks (250 ppi, CMYK/spot, hairline, registration, safe zone).".into(),
            min_ppi: 250.0,
            safe_margin_pt: 9.0,
            disallow_rgb: true,
            rgb_is_error: true,
            check_registration: true,
            check_non_proportional_scaling: true,
            min_stroke_pt: 0.25,
            ..Self::basic()
        }
    }

    pub fn pdfx1a() -> Self {
        Self {
            name: "PDF/X-1a".into(),
            description: "ISO 15930-4 blind exchange checks (no RGB, no live transparency, 300 ppi).".into(),
            min_ppi: 300.0,
            disallow_rgb: true,
            rgb_is_error: true,
            check_registration: true,
            min_stroke_pt: 0.125,
            disallow_transparency: true,
            ..Self::basic()
        }
    }

    pub fn pdfx3() -> Self {
        Self {
            name: "PDF/X-3".into(),
            description: "ISO 15930-6 color-managed print checks (no live transparency, ICC RGB allowed, 300 ppi).".into(),
            min_ppi: 300.0,
            disallow_rgb: false,
            check_registration: true,
            min_stroke_pt: 0.125,
            disallow_transparency: true,
            ..Self::basic()
        }
    }

    pub fn pdfx4() -> Self {
        Self {
            name: "PDF/X-4".into(),
            description: "ISO 15930-7 modern print checks (live transparency allowed, 300 ppi, hairline & registration).".into(),
            min_ppi: 300.0,
            check_registration: true,
            min_stroke_pt: 0.125,
            ..Self::basic()
        }
    }

    pub fn builtins() -> Vec<Self> {
        vec![
            Self::basic(),
            Self::digital_publishing(),
            Self::press_quality_cmyk(),
            Self::pdfx1a(),
            Self::pdfx3(),
            Self::pdfx4(),
        ]
    }
}

/// Resolves `name` (or the session's active profile when `None`) against custom + built-in profiles.
pub fn resolve_profile(s: &Session, name: Option<&str>) -> Option<PreflightProfile> {
    let target = name.unwrap_or(&s.active_preflight_profile);
    s.preflight_profiles
        .iter()
        .find(|p| p.name == target)
        .cloned()
        .or_else(|| PreflightProfile::builtins().into_iter().find(|p| p.name == target))
}

/// Serializes a [`PreflightProfile`] to InDesign `.idpp` XML.
pub fn profile_to_idpp_xml(p: &PreflightProfile) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <?aid style=\"50\" type=\"preflightProfile\" readerVersion=\"6.0\" featureSet=\"257\" product=\"20.0\"?>\n\
         <PreflightProfile Name=\"{}\" Description=\"{}\">\n\
           <PreflightRule Name=\"ADBE_MissingModifiedGraphics\" Flag=\"{}\" Modified=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_OversetText\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_MissingFonts\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_MissingGlyph\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_EmptyTextFrame\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_ImageResolution\" MinPPI=\"{:.2}\"/>\n\
           <PreflightRule Name=\"ADBE_BleedTrimHazard\" Flag=\"{}\" LiveAreaMarginPt=\"{:.2}\"/>\n\
           <PreflightRule Name=\"ADBE_ColorSpaces\" DisallowRGB=\"{}\" RGBError=\"{}\" DisallowSpot=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_RegistrationColor\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_WhiteOverprint\" Flag=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_ScaledGraphics\" NonProportional=\"{}\"/>\n\
           <PreflightRule Name=\"ADBE_StrokeRequirements\" MinWeightPt=\"{:.3}\"/>\n\
           <PreflightRule Name=\"ADBE_TransparencyUsage\" Disallow=\"{}\"/>\n\
         </PreflightProfile>\n",
        esc(&p.name),
        esc(&p.description),
        p.check_links,
        p.check_modified_links,
        p.check_overset,
        p.check_missing_fonts,
        p.check_missing_glyphs,
        p.check_empty_text,
        p.min_ppi,
        p.check_bleed,
        p.safe_margin_pt,
        p.disallow_rgb,
        p.rgb_is_error,
        p.disallow_spot,
        p.check_registration,
        p.check_white_overprint,
        p.check_non_proportional_scaling,
        p.min_stroke_pt,
        p.disallow_transparency,
    )
}

///Exports `.idpp` bytes (ZIP package containing `PreflightProfile.xml`, as InDesign packages `.idpp`).
pub fn write_idpp_bytes(p: &PreflightProfile) -> Vec<u8> {
    let xml = profile_to_idpp_xml(p);
    let parts = vec![("PreflightProfile.xml".to_string(), xml.as_bytes().to_vec())];
    craft_idml::write_package(&parts).unwrap_or_else(|_| xml.into_bytes())
}

/// Parses an `.idpp` profile from either a ZIP-packaged `.idpp` archive or raw `.idpp` XML bytes.
pub fn parse_idpp_bytes(bytes: &[u8], fallback_name: &str) -> std::result::Result<PreflightProfile, String> {
    let xml = if bytes.starts_with(b"PK\x03\x04") {
        use std::io::Read as _;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("invalid .idpp archive: {e}"))?;
        let mut found = None;
        for idx in 0..archive.len() {
            let mut file = archive.by_index(idx).map_err(|e| format!("invalid .idpp entry: {e}"))?;
            if file.name().ends_with(".xml") {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).map_err(|e| format!("failed reading .idpp XML: {e}"))?;
                found = Some(buf);
                break;
            }
        }
        let xml_bytes = found.ok_or_else(|| "no XML profile found inside .idpp package".to_string())?;
        String::from_utf8(xml_bytes).map_err(|e| format!("invalid UTF-8 in .idpp XML: {e}"))?
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    parse_idpp_xml(&xml, fallback_name)
}

pub fn parse_idpp_xml(xml: &str, fallback_name: &str) -> std::result::Result<PreflightProfile, String> {
    let opts = craft_xml::Options::default();
    let root = opts.parse(xml.as_bytes()).map_err(|e| format!("invalid .idpp XML: {e}"))?;
    if root.name != "PreflightProfile" {
        return Err(format!("expected <PreflightProfile>, found <{}>", root.name));
    }
    fn attr<'a>(el: &'a craft_xml::El, k: &str) -> Option<&'a str> {
        el.attrs.iter().find(|(name, _)| name == k).map(|(_, val)| val.as_str())
    }
    let mut p = PreflightProfile::basic();
    p.name = attr(&root, "Name")
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    if let Some(desc) = attr(&root, "Description") {
        p.description = desc.to_string();
    }
    let b_attr = |el: &craft_xml::El, k: &str| -> Option<bool> {
        match attr(el, k)?.to_ascii_lowercase().as_str() {
            "true" | "1" | "flag_error" | "flag_warning" => Some(true),
            "false" | "0" | "flag_off" => Some(false),
            _ => None,
        }
    };
    for node in &root.children {
        let craft_xml::Node::El(child) = node else {
            continue;
        };
        if child.name != "PreflightRule" {
            continue;
        }
        match attr(child, "Name").unwrap_or("") {
            "ADBE_MissingModifiedGraphics" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_links = v;
                }
                if let Some(v) = b_attr(child, "Modified") {
                    p.check_modified_links = v;
                }
            }
            "ADBE_OversetText" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_overset = v;
                }
            }
            "ADBE_MissingFonts" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_missing_fonts = v;
                }
            }
            "ADBE_MissingGlyph" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_missing_glyphs = v;
                }
            }
            "ADBE_EmptyTextFrame" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_empty_text = v;
                }
            }
            "ADBE_ImageResolution" => {
                if let Some(v) = attr(child, "MinPPI").and_then(|s| s.parse::<f64>().ok()) {
                    p.min_ppi = v.clamp(0.0, 4800.0);
                }
            }
            "ADBE_BleedTrimHazard" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_bleed = v;
                }
                if let Some(v) = attr(child, "LiveAreaMarginPt").and_then(|s| s.parse::<f64>().ok()) {
                    p.safe_margin_pt = v.max(0.0);
                }
            }
            "ADBE_ColorSpaces" => {
                if let Some(v) = b_attr(child, "DisallowRGB") {
                    p.disallow_rgb = v;
                }
                if let Some(v) = b_attr(child, "RGBError") {
                    p.rgb_is_error = v;
                }
                if let Some(v) = b_attr(child, "DisallowSpot") {
                    p.disallow_spot = v;
                }
            }
            "ADBE_RegistrationColor" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_registration = v;
                }
            }
            "ADBE_WhiteOverprint" => {
                if let Some(v) = b_attr(child, "Flag") {
                    p.check_white_overprint = v;
                }
            }
            "ADBE_ScaledGraphics" => {
                if let Some(v) = b_attr(child, "NonProportional") {
                    p.check_non_proportional_scaling = v;
                }
            }
            "ADBE_StrokeRequirements" => {
                if let Some(v) = attr(child, "MinWeightPt").and_then(|s| s.parse::<f64>().ok()) {
                    p.min_stroke_pt = v.max(0.0);
                }
            }
            "ADBE_TransparencyUsage" => {
                if let Some(v) = b_attr(child, "Disallow") {
                    p.disallow_transparency = v;
                }
            }
            _ => {}
        }
    }
    Ok(p)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "preflight.run", "Preflight Document", ["Window", "Output", "Preflight"], None,
            "{minPpi?: 150, profile?: str, pages?: \"1-3,5\" | [1,3]} → {profile, errors, warnings, issues: [{severity, category, kind, message, fix, item?, page?}]}",
            has_doc, run),
        cmd!(query "preflight.profiles", "Preflight Profiles", ["Window", "Output"], None,
            "{} → {active, profiles: [{name, description, builtin, minPpi, ...}]}",
            always, |s, _| {
                let mut list = Vec::new();
                for p in PreflightProfile::builtins() {
                    let mut v = serde_json::to_value(&p).unwrap_or_default();
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("builtin".into(), Value::Bool(true));
                    }
                    list.push(v);
                }
                for p in &s.preflight_profiles {
                    let mut v = serde_json::to_value(p).unwrap_or_default();
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("builtin".into(), Value::Bool(false));
                    }
                    list.push(v);
                }
                Ok(json!({"active": s.active_preflight_profile, "profiles": list}))
            }),
        cmd!(noundo "preflight.setProfile", "Select Preflight Profile", [], None,
            "{name} → {active}",
            always, |s, p| {
                let name = str_param(p, "name").ok_or_else(|| bad("preflight.setProfile", "`name` required"))?;
                if resolve_profile(s, Some(name)).is_none() {
                    return Err(bad("preflight.setProfile", format!("unknown preflight profile `{name}`")));
                }
                s.active_preflight_profile = name.to_string();
                Ok(json!({"active": s.active_preflight_profile}))
            }),
        cmd!(noundo "preflight.saveProfile", "Define Preflight Profile…", [], None,
            "{name, description?, checkLinks?, checkModifiedLinks?, checkOverset?, checkMissingFonts?, checkMissingGlyphs?, checkEmptyText?, minPpi?, checkBleed?, safeMarginPt?, disallowRgb?, rgbIsError?, disallowSpot?, checkRegistration?, checkWhiteOverprint?, checkNonProportionalScaling?, minStrokePt?, disallowTransparency?} → {profile}",
            always, |s, p| {
                let name = str_param(p, "name").filter(|n| !n.trim().is_empty()).ok_or_else(|| bad("preflight.saveProfile", "`name` required"))?.to_string();
                if PreflightProfile::builtins().iter().any(|b| b.name == name) {
                    return Err(bad("preflight.saveProfile", format!("cannot overwrite built-in profile `{name}`")));
                }
                let mut prof: PreflightProfile = serde_json::from_value(p.clone()).map_err(|e| bad("preflight.saveProfile", e.to_string()))?;
                prof.name = name;
                let js = serde_json::to_value(&prof).unwrap_or_default();
                if let Some(existing) = s.preflight_profiles.iter_mut().find(|x| x.name == prof.name) {
                    *existing = prof;
                } else {
                    s.preflight_profiles.push(prof);
                }
                Ok(json!({"profile": js}))
            }),
        cmd!(noundo "preflight.deleteProfile", "Delete Preflight Profile", [], None,
            "{name} → {deleted}",
            always, |s, p| {
                let name = str_param(p, "name").ok_or_else(|| bad("preflight.deleteProfile", "`name` required"))?;
                let before = s.preflight_profiles.len();
                s.preflight_profiles.retain(|x| x.name != name);
                if s.active_preflight_profile == name {
                    s.active_preflight_profile = "[Basic]".into();
                }
                Ok(json!({"deleted": s.preflight_profiles.len() < before}))
            }),
        cmd!(noundo "preflight.importProfile", "Load Preflight Profile (.idpp)…", [], None,
            "{path?, base64?, xml?, name?} → {profile}",
            always, |s, p| {
                let fallback = str_param(p, "name")
                    .or_else(|| str_param(p, "path").and_then(|pt| std::path::Path::new(pt).file_stem().and_then(|st| st.to_str())))
                    .unwrap_or("Imported Profile");
                let mut prof = if let Some(xml) = str_param(p, "xml") {
                    parse_idpp_xml(xml, fallback).map_err(|e| bad("preflight.importProfile", e))?
                } else if let Some(b64) = str_param(p, "base64") {
                    let bytes = super::file::base64_decode(b64);
                    parse_idpp_bytes(&bytes, fallback).map_err(|e| bad("preflight.importProfile", e))?
                } else if let Some(path) = str_param(p, "path") {
                    #[cfg(not(target_arch = "wasm32"))]
                    let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
                    #[cfg(target_arch = "wasm32")]
                    let bytes: Vec<u8> = {
                        let _ = path;
                        return Err(EngineError::Other("importing .idpp from disk requires a file system".into()));
                    };
                    parse_idpp_bytes(&bytes, fallback).map_err(|e| bad("preflight.importProfile", e))?
                } else {
                    return Err(bad("preflight.importProfile", "`path`, `base64`, or `xml` required"));
                };
                if let Some(explicit) = str_param(p, "name").filter(|n| !n.trim().is_empty()) {
                    prof.name = explicit.to_string();
                }
                if PreflightProfile::builtins().iter().any(|b| b.name == prof.name) {
                    prof.name = format!("{} (Imported)", prof.name);
                }
                let js = serde_json::to_value(&prof).unwrap_or_default();
                if let Some(existing) = s.preflight_profiles.iter_mut().find(|x| x.name == prof.name) {
                    *existing = prof;
                } else {
                    s.preflight_profiles.push(prof);
                }
                Ok(json!({"profile": js}))
            }),
        cmd!(noundo "preflight.exportProfile", "Save Preflight Profile (.idpp)…", [], None,
            "{name?, path?} → {path?, base64, xml, bytes}",
            always, |s, p| {
                let prof = resolve_profile(s, str_param(p, "name"))
                    .ok_or_else(|| bad("preflight.exportProfile", "unknown preflight profile"))?;
                let xml = profile_to_idpp_xml(&prof);
                let bytes = write_idpp_bytes(&prof);
                if let Some(path) = str_param(p, "path") {
                    #[cfg(not(target_arch = "wasm32"))]
                    crate::io::write_file(path, &bytes).map_err(EngineError::Other)?;
                    Ok(json!({"path": path, "base64": super::file::base64_encode(&bytes), "xml": xml, "bytes": bytes.len()}))
                } else {
                    Ok(json!({"base64": super::file::base64_encode(&bytes), "xml": xml, "bytes": bytes.len()}))
                }
            }),
        cmd!(noundo "preflight.exportReport", "Save Preflight Report…", [], None,
            "{path?, profile?: str, pages?: \"1-3,5\" | [1,3]} → {path?, text, errors, warnings}",
            has_doc, |s, p| {
                let val = run(s, p)?;
                let errors = val["errors"].as_u64().unwrap_or(0);
                let warnings = val["warnings"].as_u64().unwrap_or(0);
                let prof_name = val["profile"].as_str().unwrap_or("[Basic]");
                let doc_title = s.active().map(|st| st.title()).unwrap_or_default();
                let mut text = format!(
                    "DesignCraft Preflight Report\nDocument: {doc_title}\nProfile: {prof_name}\nSummary: {errors} error(s), {warnings} warning(s)\n\n"
                );
                if let Some(arr) = val["issues"].as_array() {
                    for it in arr {
                        let sev = it["severity"].as_str().unwrap_or("warning").to_ascii_uppercase();
                        let cat = it["category"].as_str().unwrap_or("DOCUMENT");
                        let msg = it["message"].as_str().unwrap_or("");
                        let pg = it["page"].as_u64().map(|p| format!(" (Page {})", p + 1)).unwrap_or_default();
                        let fix = it["fix"].as_str().unwrap_or("");
                        text.push_str(&format!("[{sev}] [{cat}] {msg}{pg}\n  Fix: {fix}\n"));
                    }
                }
                if let Some(path) = str_param(p, "path") {
                    #[cfg(not(target_arch = "wasm32"))]
                    crate::io::write_file(path, text.as_bytes()).map_err(EngineError::Other)?;
                    Ok(json!({"path": path, "text": text, "errors": errors, "warnings": warnings}))
                } else {
                    Ok(json!({"text": text, "errors": errors, "warnings": warnings}))
                }
            }),
    ]
}

fn page_of(d: &Document, sr: SpreadRef, it: &Item, parent: Affine) -> Option<usize> {
    let SpreadRef::Doc(si) = sr else { return None };
    let sp = d.spreads.get(si)?;
    let bounds = parent.transform_rect_bbox(it.bounds());
    Some(d.first_page_of_spread(si) + sp.page_at_x(bounds.center().x).unwrap_or(0))
}

fn pages_of(d: &Document, sr: SpreadRef, it: &Item, parent: Affine) -> Vec<usize> {
    let SpreadRef::Doc(si) = sr else { return vec![] };
    let Some(sp) = d.spreads.get(si) else { return vec![] };
    let bounds = parent.transform_rect_bbox(it.bounds());
    let first = d.first_page_of_spread(si);
    let touched: Vec<usize> =
        sp.pages.iter().enumerate().filter(|(_, pg)| bounds.x1 > pg.x && bounds.x0 < pg.x + pg.width).map(|(i, _)| first + i).collect();
    if touched.is_empty() { page_of(d, sr, it, parent).into_iter().collect() } else { touched }
}

const MISSING_GLYPHS_LISTED: usize = 20;

fn missing_glyphs(
    cs: &ComposedStory,
    owner: Option<designcraft_doc::ItemId>,
    depth: usize,
    out: &mut Vec<(FaceRef, Vec<char>, designcraft_doc::ItemId)>,
) {
    if depth > 16 {
        return;
    }
    for f in &cs.frames {
        let owner = owner.unwrap_or(f.frame);
        for g in f.lines.iter().flat_map(|l| &l.glyphs).filter(|g| g.visible && (g.gid == 0 || g.face.is_placeholder())) {
            let c = g.rendered_char;
            if c.is_control() || c.is_whitespace() || ('\u{E000}'..='\u{E1FF}').contains(&c) {
                continue;
            }
            match out.iter_mut().find(|(face, _, item)| *face == g.face && *item == owner) {
                Some((_, chars, _)) if !chars.contains(&c) => chars.push(c),
                Some(_) => {}
                None => out.push((g.face, vec![c], owner)),
            }
        }
        for c in f.tables.iter().flat_map(|t| &t.cells) {
            missing_glyphs(&c.text, Some(owner), depth + 1, out);
        }
        for n in &f.notes {
            missing_glyphs(&n.text, Some(owner), depth + 1, out);
        }
    }
}

type Located = (designcraft_doc::ItemId, Option<usize>, Vec<usize>);

fn overset_location(d: &Document, cache: &Cache, target: designcraft_doc::ItemId) -> Option<Located> {
    let at = |id: designcraft_doc::ItemId, loc: &designcraft_doc::ItemLoc| {
        let it = d.item(id);
        let xf = d.parent_xf(loc);
        (id, it.and_then(|it| page_of(d, loc.spread, it, xf)), it.map(|it| pages_of(d, loc.spread, it, xf)).unwrap_or_default())
    };
    if let Some(loc) = d.find(target) {
        return Some(at(target, &loc));
    }
    fn contains(root: &designcraft_compose::FrameText, target: designcraft_doc::ItemId) -> bool {
        let mut frames = vec![root];
        while let Some(frame) = frames.pop() {
            for object in &frame.objects {
                let mut found = false;
                object.object.item.walk(&mut |it| {
                    found |= it.id == target;
                });
                if found {
                    return true;
                }
            }
            frames.extend(frame.notes.iter().flat_map(|n| n.text.frames.iter()));
            frames.extend(frame.tables.iter().flat_map(|t| t.cells.iter().flat_map(|c| c.text.frames.iter())));
        }
        false
    }
    let mut current = target;
    let mut seen = std::collections::HashSet::new();
    let embedded = d.anchored_items();
    while seen.insert(current) {
        let owner = embedded.iter().find_map(|(owner, it)| (it.id == current).then_some(*owner))?;
        let cs = cache.get(d, owner, None);
        let frame = cs.frames.iter().find(|f| contains(f, current))?;
        current = frame.frame;
        if let Some(loc) = d.find(current) {
            return Some(at(current, &loc));
        }
    }
    None
}

const MAX_WARNED_PAGES: usize = 10;

fn page_list(pages: &[usize]) -> String {
    let shown: Vec<String> = pages.iter().take(MAX_WARNED_PAGES).map(|p| (p + 1).to_string()).collect();
    let rest = pages.len() - shown.len();
    let list = shown.join(", ");
    match (pages.len(), rest) {
        (1, _) => format!("page {list}"),
        (_, 0) => format!("pages {list}"),
        _ => format!("pages {list} and {rest} more"),
    }
}

fn parent_page_of(d: &Document, target: designcraft_doc::ItemId) -> Option<(usize, usize)> {
    let loc = d.find(target)?;
    let SpreadRef::Parent(pi) = loc.spread else { return None };
    let bounds = d.parent_xf(&loc).transform_rect_bbox(d.item(target)?.bounds());
    Some((pi, d.spread(loc.spread)?.page_at_x(bounds.center().x).unwrap_or(0)))
}

fn pages_showing(d: &Document, (pi, pp): (usize, usize)) -> Vec<usize> {
    (0..d.page_count())
        .filter(|p| d.page_loc(*p).is_some_and(|(si, pgi)| d.shown_parent_pages(SpreadRef::Doc(si), pgi).contains(&(pi, pp))))
        .collect()
}

fn overset_chars(story: &designcraft_doc::Story, cs: &ComposedStory) -> usize {
    story.text.get(cs.overset_at.unwrap_or(0)..).unwrap_or("").chars().count()
}

pub fn overset_warnings(s: &Session, pages: Option<&[usize]>) -> Vec<String> {
    let Some(st) = s.active() else { return vec![] };
    let d = &st.doc;
    let mut out = Vec::new();
    for story in d.stories.values() {
        let last = story.frames.last().copied();
        if let Some(parent) = last.and_then(|f| parent_page_of(d, f)) {
            let mut by_count: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
            for page in pages_showing(d, parent) {
                if pages.is_some_and(|sel| !sel.contains(&page)) {
                    continue;
                }
                let cs = s.cache.get(d, story.id, Some(page));
                let n = overset_chars(story, &cs);
                if cs.is_overset() && n > 0 {
                    by_count.entry(n).or_default().push(page);
                }
            }
            let item = last.map_or(0, |f| f.0);
            out.extend(
                by_count.into_iter().map(|(n, ps)| {
                    format!("{}: {n} character(s) of the story in frame {item} do not fit it and are not in the export", page_list(&ps))
                }),
            );
            continue;
        }
        let cs = s.cache.get(d, story.id, None);
        if !cs.is_overset() {
            continue;
        }
        let n = overset_chars(story, &cs);
        if n == 0 {
            continue;
        }
        if story.frames.is_empty() {
            out.push(format!("a story with no text frame: {n} character(s) of it are not in the export"));
            continue;
        }
        let location = last.and_then(|f| overset_location(d, &s.cache, f));
        let centre = location.as_ref().and_then(|(_, page, _)| *page);
        let touched: &[usize] = location.as_ref().map(|(_, _, ps)| ps.as_slice()).unwrap_or_default();
        let page = match pages {
            None => centre,
            Some(sel) => {
                let exported: Vec<usize> = touched.iter().copied().filter(|p| sel.contains(p)).collect();
                match centre.filter(|p| sel.contains(p)).or_else(|| exported.first().copied()) {
                    Some(p) => Some(p),
                    None if centre.is_some() || !touched.is_empty() => continue,
                    None => None,
                }
            }
        };
        let item = location.as_ref().map(|(item, _, _)| *item).or(last);
        out.push(match (page, item) {
            (Some(page), Some(item)) => {
                format!("page {}: {n} character(s) of the story in frame {} do not fit it and are not in the export", page + 1, item.0)
            }
            (None, Some(item)) => format!("the story in frame {}: {n} character(s) do not fit and are not in the export", item.0),
            _ => format!("{n} character(s) of a story do not fit their frame and are not in the export"),
        });
    }
    out
}

pub fn check(s: &Session, min_ppi: f64) -> Vec<Issue> {
    let Some(st) = s.active() else { return vec![] };
    let mut prof = resolve_profile(s, None).unwrap_or_default();
    prof.min_ppi = min_ppi;
    check_document_with_profile(&st.doc, &s.cache, st.path.as_deref().map(std::path::Path::new), &prof)
}

/// [`check`] of document `d` saved at `document` (relative links resolve against it), with the
/// compositions in `cache`. It needs no session, so it can run on another thread.
pub fn check_document(d: &Document, cache: &Cache, document: Option<&std::path::Path>, min_ppi: f64) -> Vec<Issue> {
    let mut prof = PreflightProfile::basic();
    prof.min_ppi = min_ppi;
    check_document_with_profile(d, cache, document, &prof)
}

/// Evaluates `d` against `profile`, returning structured issues with category and fix hints.
pub fn check_document_with_profile(
    d: &Document,
    cache: &Cache,
    document: Option<&std::path::Path>,
    profile: &PreflightProfile,
) -> Vec<Issue> {
    let min_ppi = profile.min_ppi;
    let mut out = Vec::new();
    let mut missing_fonts: Vec<String> = Vec::new();
    let mut unsupported_typography = std::collections::BTreeSet::new();
    let mut missing: Vec<(FaceRef, Vec<char>, designcraft_doc::ItemId)> = Vec::new();
    let mut white_text: std::collections::BTreeMap<Option<designcraft_doc::ItemId>, Vec<&'static str>> = std::collections::BTreeMap::new();

    for story in d.stories.values() {
        let cs = cache.get(d, story.id, None);
        if profile.check_missing_glyphs {
            missing_glyphs(&cs, None, 0, &mut missing);
        }
        if profile.check_overset && cs.is_overset() {
            let last = story.frames.last().copied();
            let location = last.and_then(|f| overset_location(d, cache, f));
            let n = story.text.get(cs.overset_at.unwrap_or(0)..).unwrap_or("").chars().count();
            let unplaced = story.frames.is_empty();
            out.push(Issue {
                severity: if unplaced { "warning" } else { "error" },
                category: "TEXT",
                kind: if unplaced { "unplacedStory" } else { "overset" },
                message: if unplaced {
                    format!("Unplaced story {}: {n} characters have no text frame", story.id)
                } else {
                    format!("Overset text: {n} characters")
                },
                fix: if unplaced {
                    "Place the story into a text frame or delete the unused story."
                } else {
                    "Enlarge the text frame, thread it to another frame, or reduce text size."
                },
                item: location.as_ref().map(|(item, _, _)| item.0).or(last.map(|i| i.0)),
                page: location.as_ref().and_then(|(_, page, _)| *page),
            });
        }
    }

    super::fonts::for_each_story(d, &mut |story| {
        let ranges = story.para_ranges();
        let mut shown = None;
        for (pi, pf) in story.paras.iter().enumerate() {
            let (para, base) = d.styles.resolve_para(pf);
            let Some(range) = ranges.get(pi) else { continue };
            let has_rtl = story.text.get(range.clone()).is_some_and(|text| text.chars().any(designcraft_fonts::is_rtl));
            if has_rtl {
                if !matches!(para.arabic_justification.as_str(), "" | "DefaultJustification") {
                    unsupported_typography.insert(format!(
                        "Arabic justification policy `{}` is preserved; vendor-specific Naskh/Arabic algorithms are not implemented",
                        para.arabic_justification
                    ));
                }
                if para.paragraph_kashida_width.is_some() && para.arabic_justification != "DefaultJustification" {
                    unsupported_typography
                        .insert("Paragraph Kashida width preset is preserved; automatic elongation uses the engine's bounded allocation".into());
                }
            }
            match designcraft_doc::mojikumi::Rules::resolve(&d.styles, &para.mojikumi) {
                Err(reason) => {
                    unsupported_typography.insert(format!("Mojikumi `{}` is not applied: {reason}", para.mojikumi));
                }
                Ok(Some(rules)) if rules.assumed_base() => {
                    unsupported_typography
                        .insert(format!("Mojikumi `{}` names no base set; its rules apply on the default half-em set", para.mojikumi));
                }
                Ok(_) => {}
            }
            if !matches!(
                para.kinsoku_type.as_str(),
                "" | "KinsokuPushInFirst" | "KinsokuPushOutFirst" | "KinsokuPushOutOnly" | "KinsokuPrioritizeAdjustmentAmount"
            ) {
                unsupported_typography
                    .insert(format!("Kinsoku priority `{}` is preserved, but push-in/push-out priority is not applied", para.kinsoku_type));
            }
            for (run, f) in story.runs().filter(|(run, _)| run.start < range.end && run.end > range.start) {
                let props = d.styles.resolve_char(&base, f);
                if profile.check_white_overprint {
                    let white = white_overprint::adornments(d, &props);
                    if !white.is_empty() {
                        let shown = shown.get_or_insert_with(|| white_overprint::frame_ranges(cache, d, story));
                        for frame in white_overprint::frames_showing(story, shown, run.start.max(range.start)..run.end.min(range.end)) {
                            let parts = white_text.entry(frame).or_default();
                            for part in &white {
                                if !parts.contains(part) {
                                    parts.push(part);
                                }
                            }
                        }
                    }
                }
                if has_rtl
                    && story.text.get(run.start.max(range.start)..run.end.min(range.end)).is_some_and(|t| t.chars().any(designcraft_fonts::is_rtl))
                {
                    use designcraft_doc::arabic::DiacriticPosition as P;
                    if !matches!(props.diacritic_position, P::Default | P::OpenType) {
                        unsupported_typography.insert(format!(
                            "Diacritic preset `{:?}` is preserved; font OpenType anchors and explicit offsets are used",
                            props.diacritic_position
                        ));
                    }
                    if !matches!(props.positional_form.as_str(), "" | "None" | "Calculate" | "Initial" | "Medial" | "Final" | "Isolated") {
                        unsupported_typography.insert(format!("Unknown positional form `{}` is preserved but not applied", props.positional_form));
                    }
                }
            }
        }
    });

    if profile.check_missing_fonts {
        for font in super::fonts::used_fonts(d) {
            if font.style_missing {
                out.push(Issue {
                    severity: "error",
                    category: "TEXT",
                    kind: "missingFontStyle",
                    message: format!("Missing font style: {} {} (using {} {})", font.family, font.style, font.resolved_family, font.resolved_style),
                    fix: "Install the missing font weight/style or replace it via Type › Find/Replace Font.",
                    item: None,
                    page: None,
                });
            }
            if font.missing && !missing_fonts.contains(&font.family) {
                missing_fonts.push(font.family);
            }
        }
        for f in missing_fonts {
            out.push(Issue {
                severity: "error",
                category: "TEXT",
                kind: "missingFont",
                message: format!("Missing font: {f}"),
                fix: "Install the missing font family or substitute an available font via Type › Find/Replace Font.",
                item: None,
                page: None,
            });
        }
    }

    for message in unsupported_typography {
        out.push(Issue {
            severity: "warning",
            category: "TEXT",
            kind: "unsupportedTypography",
            message,
            fix: "Switch to a supported justification, mojikumi, kinsoku, or OpenType diacritic setting.",
            item: None,
            page: None,
        });
    }

    if profile.check_white_overprint {
        let (white_items, printed_frames) = white_overprint::printed(cache, d);
        for (item, parts) in white_text {
            let page = match item {
                Some(id) => match printed_frames.get(&id) {
                    Some(page) => *page,
                    None => continue,
                },
                None => None,
            };
            out.extend(white_overprint::issue(&parts, item.map(|i| i.0), page));
        }
        out.extend(white_items);
    }

    for (face, chars, item) in missing {
        let listed: Vec<String> = chars.iter().take(MISSING_GLYPHS_LISTED).map(char::to_string).collect();
        let more = chars.len().saturating_sub(MISSING_GLYPHS_LISTED);
        let more = if more > 0 { format!(" and {more} more") } else { String::new() };
        out.push(Issue {
            severity: "error",
            category: "TEXT",
            kind: "missingGlyphs",
            message: format!("Missing glyphs in {} {}: {}{more}", face.family, face.style, listed.join(" ")),
            fix: "Choose a font containing these Unicode characters or enable Document Preferences › Glyph Fallback.",
            item: Some(item.0),
            page: d.find(item).zip(d.item(item)).and_then(|(loc, it)| page_of(d, loc.spread, it, d.parent_xf(&loc))),
        });
    }

    for (si, sp) in d.spreads.iter().enumerate() {
        let trim_bounds = sp.bounds();
        let bleed_box = sp.bleed_rect(d.settings.bleed);
        let allowed_bounds = bleed_box.unwrap_or(trim_bounds);
        let mut pending: Vec<_> = sp.items.iter().rev().map(|it| (it.as_ref(), Affine::IDENTITY, false)).collect();
        while let Some((it, parent, clipped)) = pending.pop() {
            let xf = parent * it.xf;
            let clips_children = clipped || !it.is_group();
            pending.extend(it.children().iter().rev().map(|child| (child.as_ref(), xf, clips_children)));
            if it.hidden {
                continue;
            }
            let page = page_of(d, SpreadRef::Doc(si), it, parent);
            let bounds = parent.transform_rect_bbox(it.bounds());
            let eps = 1e-3;
            let on_pages = bounds.x1 >= trim_bounds.x0 - eps
                && bounds.x0 <= trim_bounds.x1 + eps
                && bounds.y1 >= trim_bounds.y0 - eps
                && bounds.y0 <= trim_bounds.y1 + eps;

            if profile.check_bleed && !it.is_group() && !clipped && !it.nonprinting && on_pages && !trim_bounds.is_zero_area() {
                let outside_allowed = bounds.x0 < allowed_bounds.x0 - eps
                    || bounds.y0 < allowed_bounds.y0 - eps
                    || bounds.x1 > allowed_bounds.x1 + eps
                    || bounds.y1 > allowed_bounds.y1 + eps;
                if outside_allowed {
                    out.push(Issue {
                        severity: "error",
                        category: "DOCUMENT",
                        kind: "outOfBleed",
                        message: if bleed_box.is_some() {
                            "Frame extends outside bleed area".into()
                        } else {
                            "Frame extends outside page trim area".into()
                        },
                        fix: "Resize or snap the frame to the document bleed/trim guide.",
                        item: Some(it.id.0),
                        page,
                    });
                } else if let Some(b) = bleed_box {
                    let incomplete_bleed = (bounds.x0 < trim_bounds.x0 - eps && bounds.x0 > b.x0 + eps)
                        || (bounds.y0 < trim_bounds.y0 - eps && bounds.y0 > b.y0 + eps)
                        || (bounds.x1 > trim_bounds.x1 + eps && bounds.x1 < b.x1 - eps)
                        || (bounds.y1 > trim_bounds.y1 + eps && bounds.y1 < b.y1 - eps);
                    if incomplete_bleed {
                        out.push(Issue {
                            severity: "warning",
                            category: "DOCUMENT",
                            kind: "bleedHazard",
                            message: "Frame crosses trim edge without reaching full bleed".into(),
                            fix: "Extend the frame all the way to the bleed guide or pull it inside the page margin.",
                            item: Some(it.id.0),
                            page,
                        });
                    }
                }
            }

            // Trim safe zone (live area) check for objects inside trim that sit dangerously close to trim.
            if profile.safe_margin_pt > 0.0
                && !it.is_group()
                && !clipped
                && !it.nonprinting
                && on_pages
                && let Some(pi) = sp.page_at_x(bounds.center().x)
                && let Some(pg) = sp.pages.get(pi)
            {
                let pb = pg.bounds();
                let inside_trim = bounds.x0 >= pb.x0 - eps
                    && bounds.y0 >= pb.y0 - eps
                    && bounds.x1 <= pb.x1 + eps
                    && bounds.y1 <= pb.y1 + eps;
                let safe = pb.inflate(-profile.safe_margin_pt, -profile.safe_margin_pt);
                if inside_trim
                    && safe.width() > 0.0
                    && safe.height() > 0.0
                    && (bounds.x0 < safe.x0 - eps
                        || bounds.y0 < safe.y0 - eps
                        || bounds.x1 > safe.x1 + eps
                        || bounds.y1 > safe.y1 + eps)
                {
                    out.push(Issue {
                        severity: "warning",
                        category: "DOCUMENT",
                        kind: "trimSafeZone",
                        message: format!("Object is within {:.1} pt of the page trim edge", profile.safe_margin_pt),
                        fix: "Move critical content inside the safe margin or extend background artwork to full bleed.",
                        item: Some(it.id.0),
                        page,
                    });
                }
            }

            // [Registration] swatch misuse check on user objects.
            if profile.check_registration
                && !it.nonprinting
                && (it.fill.swatch == swatch::REGISTRATION || (!it.stroke.is_none() && it.stroke.weight > 0.0 && it.stroke.swatch == swatch::REGISTRATION))
            {
                out.push(Issue {
                    severity: "error",
                    category: "COLOR",
                    kind: "registrationColor",
                    message: "[Registration] color applied to page object (prints on all plates)".into(),
                    fix: "Use [Black] instead of [Registration] for page artwork and text.",
                    item: Some(it.id.0),
                    page,
                });
            }

            // Minimum stroke weight (hairline) check.
            if profile.min_stroke_pt > 0.0
                && !it.nonprinting
                && !it.stroke.is_none()
                && it.stroke.weight > 0.0
                && it.stroke.weight + 1e-4 < profile.min_stroke_pt
            {
                out.push(Issue {
                    severity: "warning",
                    category: "IMAGES and OBJECTS",
                    kind: "minStrokeWeight",
                    message: format!("Stroke weight {:.3} pt is below minimum {:.3} pt", it.stroke.weight, profile.min_stroke_pt),
                    fix: "Increase the stroke weight in the Stroke or Properties panel so it holds on press.",
                    item: Some(it.id.0),
                    page,
                });
            }

            // Live transparency check (for PDF/X-1a and PDF/X-3 profiles).
            if profile.disallow_transparency
                && !it.nonprinting
                && (it.opacity < 0.999
                    || it.blend != BlendMode::Normal
                    || it.isolate
                    || it.knockout
                    || it.effects != designcraft_doc::Effects::default())
            {
                out.push(Issue {
                    severity: "warning",
                    category: "IMAGES and OBJECTS",
                    kind: "liveTransparency",
                    message: "Object uses live transparency (opacity, blend mode, or soft effects)".into(),
                    fix: "Remove transparency or export with a Transparency Flattener / PDF/X-1a or PDF/X-3 preset.",
                    item: Some(it.id.0),
                    page,
                });
            }

            if let Content::Graphic(g) = &it.content
                && g.layer_options.as_ref().is_some_and(designcraft_doc::GraphicLayerOptions::has_visibility_overrides)
            {
                out.push(Issue {
                    severity: "warning",
                    category: "IMAGES and OBJECTS",
                    kind: "unsupportedGraphicLayers",
                    message: designcraft_doc::GRAPHIC_LAYER_RENDERING_WARNING.into(),
                    fix: "Export to PDF or flatten graphic layers in the source file.",
                    item: Some(it.id.0),
                    page,
                });
            }

            match &it.content {
                Content::Graphic(g) => match d.assets.get(&g.asset) {
                    None if profile.check_links => out.push(Issue {
                        severity: "error",
                        category: "LINKS",
                        kind: "missingLink",
                        message: "Missing graphic".into(),
                        fix: "Relink the missing graphic in the Links panel.",
                        item: Some(it.id.0),
                        page,
                    }),
                    None => {}
                    Some(a) if profile.check_links && (a.data.is_empty() || super::links::link_missing(a, document)) => out.push(Issue {
                        severity: "error",
                        category: "LINKS",
                        kind: "missingLink",
                        message: match a.link.as_deref().and_then(|l| super::links::refused(l, document)) {
                            Some(why) => format!("Missing link: {} ({why})", a.link.as_deref().unwrap_or(&a.name)),
                            None => format!("Missing link: {}", a.link.as_deref().unwrap_or(&a.name)),
                        },
                        fix: "Locate and relink the file in the Links panel.",
                        item: Some(it.id.0),
                        page,
                    }),
                    Some(a) => {
                        if profile.check_modified_links && link_file_modified(a, document) {
                            out.push(Issue {
                                severity: "warning",
                                category: "LINKS",
                                kind: "modifiedLink",
                                message: format!("Modified link: {}", a.link.as_deref().unwrap_or(&a.name)),
                                fix: "Click Update Link in the Links panel to refresh the placed graphic.",
                                item: Some(it.id.0),
                                page,
                            });
                        }
                        let [xx, yx, xy, yy, _, _] = (xf * g.xf).as_coeffs();
                        let sx = xx.hypot(yx);
                        let sy = xy.hypot(yy);
                        if profile.check_non_proportional_scaling && sx > 1e-6 && sy > 1e-6 {
                            let ratio = (sx / sy - 1.0).abs();
                            if ratio > 0.01 {
                                out.push(Issue {
                                    severity: "warning",
                                    category: "IMAGES and OBJECTS",
                                    kind: "nonProportionalScaling",
                                    message: format!(
                                        "{}: non-proportional scaling ({:.1}% × {:.1}%)",
                                        a.name,
                                        sx * 100.0,
                                        sy * 100.0
                                    ),
                                    fix: "Use Fit Content Proportionally in Object › Fitting to restore 1:1 aspect ratio.",
                                    item: Some(it.id.0),
                                    page,
                                });
                            }
                        }
                        let vector = designcraft_images::is_pdf(&a.data) || designcraft_images::is_svg(&a.data);
                        if min_ppi > 0.0
                            && let Some((pw, ph)) = a.pixels.filter(|_| !vector)
                        {
                            let placed_w = sx * g.size.0;
                            let placed_h = sy * g.size.1;
                            let ppi_x = pw as f64 / (placed_w / 72.0).max(1e-6);
                            let ppi_y = ph as f64 / (placed_h / 72.0).max(1e-6);
                            let ppi = ppi_x.min(ppi_y);
                            if (d.settings.intent == Intent::Print || profile.disallow_rgb || profile.disallow_transparency) && ppi < min_ppi {
                                out.push(Issue {
                                    severity: "warning",
                                    category: "IMAGES and OBJECTS",
                                    kind: "lowResolution",
                                    message: format!("{}: effective {ppi:.0} ppi (< {min_ppi:.0})", a.name),
                                    fix: "Scale the graphic down or relink to a higher-resolution source image.",
                                    item: Some(it.id.0),
                                    page,
                                });
                            }
                        }
                    }
                },
                Content::Text(tf) if profile.check_empty_text && d.story(tf.story).is_some_and(|s| s.is_empty()) => {
                    out.push(Issue {
                        severity: "warning",
                        category: "TEXT",
                        kind: "emptyText",
                        message: "Empty text frame".into(),
                        fix: "Convert the empty text frame to an unassigned/graphic frame or delete it.",
                        item: Some(it.id.0),
                        page,
                    });
                }
                _ => {}
            }
        }
    }

    if d.settings.intent == Intent::Print || profile.disallow_rgb {
        let sev = if profile.rgb_is_error { "error" } else { "warning" };
        for sw in &d.swatches {
            if let SwatchValue::Color { color: Color::Rgb { .. }, .. } = sw.value {
                out.push(Issue {
                    severity: sev,
                    category: "COLOR",
                    kind: "rgbColor",
                    message: format!("RGB swatch in a print document: {}", sw.name),
                    fix: "Convert the swatch to CMYK in the Swatches panel or export with CMYK color conversion.",
                    item: None,
                    page: None,
                });
            }
        }
    }

    if profile.disallow_spot {
        for sw in &d.swatches {
            if let SwatchValue::Color { color_type: ColorType::Spot, .. } = sw.value {
                out.push(Issue {
                    severity: "error",
                    category: "COLOR",
                    kind: "spotColor",
                    message: format!("Spot color swatch not allowed by profile: {}", sw.name),
                    fix: "Convert spot swatches to Process CMYK in the Swatches or Ink Manager panel.",
                    item: None,
                    page: None,
                });
            }
        }
    }

    if d.has_unwalked_graphic_layer_visibility_overrides() {
        out.push(Issue {
            severity: "warning",
            category: "IMAGES and OBJECTS",
            kind: "unsupportedGraphicLayers",
            message: designcraft_doc::GRAPHIC_LAYER_RENDERING_WARNING.into(),
            fix: "Export to PDF or flatten graphic layers in the source file.",
            item: None,
            page: None,
        });
    }
    out
}

fn link_file_modified(a: &designcraft_doc::Asset, document: Option<&std::path::Path>) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(link) = a.link.as_deref() else { return false };
        if super::links::refused(link, document).is_some() {
            return false;
        }
        let Ok(resolved) = crate::io::document_path(link, document) else { return false };
        let Ok(disk_bytes) = crate::io::read(&resolved) else { return false };
        disk_bytes.as_slice() != a.data.as_slice()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (a, document);
        false
    }
}

fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let mut prof = match str_param(p, "profile") {
        Some(name) => resolve_profile(s, Some(name)).ok_or_else(|| bad("preflight.run", format!("unknown preflight profile `{name}`")))?,
        None => resolve_profile(s, None).unwrap_or_default(),
    };
    if let Some(min) = p.get("minPpi").and_then(Value::as_f64) {
        prof.min_ppi = min;
    }
    let page_filter = match s.active() {
        Some(st) => crate::cmd::export::pages_param("preflight.run", p, st.doc.page_count())?,
        None => None,
    };
    let mut issues = match s.active() {
        Some(st) => check_document_with_profile(&st.doc, &s.cache, st.path.as_deref().map(std::path::Path::new), &prof),
        None => Vec::new(),
    };
    if let Some(allowed_pages) = &page_filter {
        issues.retain(|i| match i.page {
            Some(pg) => allowed_pages.contains(&pg),
            None => true,
        });
    }
    let errors = issues.iter().filter(|i| i.severity == "error").count();
    Ok(json!({
        "profile": prof.name,
        "errors": errors,
        "warnings": issues.len() - errors,
        "issues": issues,
    }))
}

#[cfg(test)]
mod placed_vector_tests {
    use serde_json::json;

    use crate::Session;
    use crate::cmd::base64_encode;

    #[test]
    fn vector_graphics_have_no_resolution() {
        let mut logo = Session::new();
        logo.execute("file.new", &json!({"width": 100, "height": 50})).unwrap();
        let pdf = logo.execute("file.exportPdf", &json!({})).unwrap();
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("file.place", &json!({"base64": pdf["base64"], "name": "logo.pdf", "x": 72, "y": 72, "width": 300})).unwrap();
        s.execute("selection.set", &json!({"ids": []})).unwrap();
        let png = designcraft_render::Rendered { width: 40, height: 20, pixels: vec![200; 40 * 20 * 4] }.to_png();
        s.execute("file.place", &json!({"base64": base64_encode(&png), "name": "photo.png", "x": 72, "y": 300, "width": 300})).unwrap();
        let r = s.execute("preflight.run", &json!({})).unwrap();
        let low: Vec<&str> =
            r["issues"].as_array().unwrap().iter().filter(|i| i["kind"] == "lowResolution").map(|i| i["message"].as_str().unwrap()).collect();
        assert_eq!(low, ["photo.png: effective 10 ppi (< 150)"]);
    }

    #[test]
    fn a_missing_link_shown_by_its_preview_is_still_reported() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let png = designcraft_render::Rendered { width: 40, height: 20, pixels: vec![200; 40 * 20 * 4] }.to_png();
        s.execute("file.place", &json!({"base64": base64_encode(&png), "name": "photo.jpg", "x": 72, "y": 72, "width": 20})).unwrap();
        let doc = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        let asset = doc.assets.values_mut().next().unwrap();
        let temp = std::path::absolute(std::env::temp_dir()).unwrap();
        let link = temp.join(format!("dc-preflight-missing-{} dir", std::process::id())).join("photo.jpg");
        let link = link.to_string_lossy().into_owned();
        std::sync::Arc::make_mut(asset).link = Some(link.clone());
        let r = s.execute("preflight.run", &json!({})).unwrap();
        let missing: Vec<&str> =
            r["issues"].as_array().unwrap().iter().filter(|i| i["kind"] == "missingLink").map(|i| i["message"].as_str().unwrap()).collect();
        assert_eq!(missing, [format!("Missing link: {link}")]);
    }
}

#[cfg(test)]
mod arabic_tests {
    use super::*;

    #[test]
    fn arabic_unsupported_presets_are_reported_without_flagging_supported_offsets() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let text = "بُسْمِ";
        let r = s.execute("frame.create", &json!({"rect": [0, 0, 300, 200], "content": "text", "text": text})).unwrap();
        s.execute("text.select", &json!({"story": r["story"], "anchor": 0, "focus": text.len()})).unwrap();
        s.execute("type.char", &json!({"attrs": {"diacriticPosition": "openType", "diacriticXOffset": 150}})).unwrap();
        assert!(!check(&s, 150.0).iter().any(|i| i.kind == "unsupportedTypography"));
        s.execute("type.para", &json!({"attrs": {"arabicJustification": "NaskhJustification", "paragraphKashidaWidth": 2}})).unwrap();
        s.execute("type.char", &json!({"attrs": {"diacriticPosition": "tight"}})).unwrap();
        let issues = check(&s, 150.0);
        let typography: Vec<_> = issues.iter().filter(|i| i.kind == "unsupportedTypography").collect();
        assert_eq!(typography.len(), 3, "{typography:?}");
        assert!(typography.iter().all(|i| i.severity == "warning"));
    }
}

#[cfg(test)]
mod missing_glyph_tests {
    use serde_json::json;

    use crate::Session;

    fn missing_glyphs(s: &mut Session) -> Vec<(String, String)> {
        let r = s.execute("preflight.run", &json!({})).unwrap();
        r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "missingGlyphs")
            .map(|i| (i["severity"].as_str().unwrap().to_string(), i["message"].as_str().unwrap().to_string()))
            .collect()
    }

    #[test]
    fn characters_drawn_as_missing_glyphs_are_listed() {
        designcraft_fonts::FontDb::global().add_font(designcraft_fonts::testing::font_with("DC Test Preflight Helper", &['日', '本', '語']).unwrap());
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": "Hello 日本語 日本 and 語."})).unwrap();
        let family = designcraft_fonts::DEFAULT_FAMILY;
        assert_eq!(missing_glyphs(&mut s), [("error".to_string(), format!("Missing glyphs in {family} Regular: 日 本 語"))]);
        s.execute("document.preferences", &json!({"glyphFallback": true})).unwrap();
        assert!(missing_glyphs(&mut s).is_empty());
    }

    #[test]
    fn a_long_list_of_missing_glyphs_is_cut_short() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let text: String = (0..30).filter_map(|i| char::from_u32(0xF0B00 + i)).collect();
        s.execute("frame.create", &json!({"rect": [72, 72, 400, 300], "content": "text", "text": text})).unwrap();
        let found = missing_glyphs(&mut s);
        assert_eq!(found.len(), 1);
        assert!(found[0].1.ends_with(" and 10 more"), "{}", found[0].1);
    }
}

#[cfg(test)]
mod generated_glyph_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn generated_suffix_missing_glyph_is_reported_at_its_page_frame() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let created = s.execute("frame.create", &json!({"rect":[20,20,300,400],"content":"text","text":"Body"})).unwrap();
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        let sid = *d.stories.keys().next().unwrap();
        d.footnote_options.affix_in = designcraft_doc::notes::AffixIn::Reference;
        d.footnote_options.suffix = "\u{10fffd}".into();
        d.story_mut(sid).unwrap().insert_note(4, "Note", designcraft_doc::ParaFormat::default());
        let report = s.execute("preflight.run", &json!({})).unwrap();
        let issue = report["issues"].as_array().unwrap().iter().find(|i| i["kind"] == "missingGlyphs").unwrap();
        assert!(issue["message"].as_str().unwrap().contains('\u{10fffd}'));
        assert_eq!(issue["item"], created["id"]);
        assert_eq!(issue["page"], 0);
    }
}

#[cfg(test)]
mod mojikumi_nested_tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn unknown_table_in_a_note_inside_a_cell_is_reported() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("frame.create", &json!({"rect":[20,20,300,400],"content":"text","text":"Body"})).unwrap();
        let d = Arc::make_mut(&mut s.doc_mut().unwrap().doc);
        let sid = *d.stories.keys().next().unwrap();
        let mut table = designcraft_doc::Table::new(1, 1, 1, 0, 0, 100.0);
        let cell = &mut table.cells[0].text;
        let mut pf = designcraft_doc::ParaFormat::default();
        pf.para.mojikumi = Some("MojikumiTable/Unrecognized".into());
        cell.insert_note(0, "Note", pf);
        d.story_mut(sid).unwrap().insert_table(0, table);
        let issues = check(&s, 150.0);
        assert_eq!(issues.iter().filter(|i| i.kind == "unsupportedTypography" && i.message.contains("Unrecognized")).count(), 1);
    }
}

#[cfg(test)]
mod overset_page_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn overset_frame_in_a_moved_group_names_its_displayed_page() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"facingPages": true, "pages": 3, "width": 612, "height": 792})).unwrap();
        let frame = json!({"spread": 1, "rect": [72, 72, 140, 100], "content": "text", "text": "overflowing words ".repeat(100)});
        let a = s.execute("frame.create", &frame).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("frame.create", &json!({"spread": 1, "rect": [300, 300, 350, 350]})).unwrap()["id"].as_u64().unwrap();
        let g = s.execute("object.group", &json!({"ids": [a, b]})).unwrap()["id"].as_u64().unwrap();
        let overset_page = |s: &mut Session| {
            let r = s.execute("preflight.run", &json!({})).unwrap();
            let issue = r["issues"].as_array().unwrap().iter().find(|i| i["kind"] == "overset").unwrap().clone();
            issue["page"].as_u64().unwrap()
        };
        assert_eq!(overset_page(&mut s), 1);
        s.execute("transform.move", &json!({"dx": 612, "dy": 0, "ids": [g]})).unwrap();
        assert_eq!(overset_page(&mut s), 2);
    }
}

#[cfg(test)]
mod bleed_and_profile_tests {
    use serde_json::json;

    use crate::Session;
    use crate::cmd::base64_decode;

    #[test]
    fn out_of_bleed_and_incomplete_bleed_frames_are_detected() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 612, "height": 792, "bleed": [9, 9, 9, 9]})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 200, 200], "content": "text", "text": "Safe body"})).unwrap();
        s.execute("frame.create", &json!({"rect": [-9, -9, 621, 801]})).unwrap();
        let clean = s.execute("preflight.run", &json!({})).unwrap();
        assert_eq!(clean["errors"], 0, "{clean:?}");
        assert!(!clean["issues"].as_array().unwrap().iter().any(|i| i["kind"] == "outOfBleed" || i["kind"] == "bleedHazard"));

        let hazard = s.execute("frame.create", &json!({"rect": [-4, 100, 120, 200]})).unwrap()["id"].as_u64().unwrap();
        let r_hazard = s.execute("preflight.run", &json!({})).unwrap();
        let hazard_issue = r_hazard["issues"].as_array().unwrap().iter().find(|i| i["kind"] == "bleedHazard").unwrap();
        assert_eq!(hazard_issue["severity"], "warning");
        assert_eq!(hazard_issue["item"], hazard);

        let out_id = s.execute("frame.create", &json!({"rect": [-25, 100, 120, 200], "content": "text", "text": "Out of bleed"})).unwrap()["id"]
            .as_u64()
            .unwrap();
        let r_out = s.execute("preflight.run", &json!({})).unwrap();
        let out_issue = r_out["issues"].as_array().unwrap().iter().find(|i| i["kind"] == "outOfBleed").unwrap();
        assert_eq!(out_issue["severity"], "error");
        assert_eq!(out_issue["item"], out_id);
        assert_eq!(out_issue["page"], 0);
        assert_eq!(r_out["errors"], 1);
    }

    #[test]
    fn pasteboard_objects_and_content_clipped_by_a_frame_are_not_bleed_errors() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 612, "height": 792, "bleed": [9, 9, 9, 9]})).unwrap();
        s.execute("frame.create", &json!({"rect": [-300, 100, -100, 200]})).unwrap();
        let big = s.execute("frame.create", &json!({"rect": [-100, -100, 400, 400]})).unwrap()["id"].as_u64().unwrap();
        let frame = s.execute("frame.create", &json!({"rect": [72, 72, 300, 300]})).unwrap()["id"].as_u64().unwrap();
        s.execute("selection.set", &json!({"ids": [big]})).unwrap();
        s.execute("edit.cut", &json!({})).unwrap();
        s.execute("selection.set", &json!({"ids": [frame]})).unwrap();
        s.execute("edit.pasteInto", &json!({})).unwrap();
        let r = s.execute("preflight.run", &json!({})).unwrap();
        let bleed: Vec<_> = r["issues"].as_array().unwrap().iter().filter(|i| i["kind"] == "outOfBleed" || i["kind"] == "bleedHazard").collect();
        assert!(bleed.is_empty(), "{bleed:?}");
        s.execute("transform.move", &json!({"dx": -100, "dy": 0, "ids": [frame]})).unwrap();
        let r = s.execute("preflight.run", &json!({})).unwrap();
        let out: Vec<_> = r["issues"].as_array().unwrap().iter().filter(|i| i["kind"] == "outOfBleed").collect();
        assert_eq!(out.len(), 1, "{r}");
        assert_eq!(out[0]["item"], frame);
    }

    #[test]
    fn idpp_profile_roundtrip_and_press_rules_detect_registration_hairline_and_transparency() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 612, "height": 792})).unwrap();
        let box_id = s.execute("frame.create", &json!({"rect": [4, 72, 120, 180]})).unwrap()["id"].as_u64().unwrap();
        {
            let d = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc);
            let it = d.item_mut(designcraft_doc::ItemId(box_id)).unwrap();
            it.fill.swatch = designcraft_color::swatch::REGISTRATION.into();
            it.stroke.swatch = designcraft_color::swatch::BLACK.into();
            it.stroke.weight = 0.1;
            it.opacity = 0.6;
        }

        // Export and re-import a custom .idpp profile.
        s.execute(
            "preflight.saveProfile",
            &json!({
                "name": "Strict Offset",
                "safeMarginPt": 12.0,
                "checkRegistration": true,
                "minStrokePt": 0.25,
                "disallowTransparency": true
            }),
        )
        .unwrap();
        let exported = s.execute("preflight.exportProfile", &json!({"name": "Strict Offset"})).unwrap();
        let b64 = exported["base64"].as_str().unwrap().to_string();
        assert!(base64_decode(&b64).starts_with(b"PK\x03\x04"));

        let mut s2 = Session::new();
        let imported = s2.execute("preflight.importProfile", &json!({"base64": b64})).unwrap();
        assert_eq!(imported["profile"]["name"], "Strict Offset");

        let report = s.execute("preflight.run", &json!({"profile": "Strict Offset"})).unwrap();
        let kinds: Vec<&str> = report["issues"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"trimSafeZone"), "{kinds:?}");
        assert!(kinds.contains(&"registrationColor"), "{kinds:?}");
        assert!(kinds.contains(&"minStrokeWeight"), "{kinds:?}");
        assert!(kinds.contains(&"liveTransparency"), "{kinds:?}");
    }
}
