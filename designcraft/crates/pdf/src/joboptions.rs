//! Adobe Distiller / InDesign `.joboptions` PDF preset parser and writer.
//!
//! `.joboptions` files are PostScript dictionary literals (`<< /Key Value ... >>`) optionally
//! followed by `setdistillerparams`. This module provides a complete recursive tokenizer/parser,
//! lossless round-trip preservation of unknown Distiller keys, built-in InDesign presets, and
//! bidirectional mapping with [`crate::PdfOptions`].

use std::collections::BTreeMap;

use crate::{Marks, PdfOptions, Standard};

/// A parsed PostScript value inside a `.joboptions` dictionary.
#[derive(Clone, Debug, PartialEq)]
pub enum JobValue {
    Bool(bool),
    Int(i64),
    Real(f64),
    /// PostScript literal name without the leading `/` (e.g. `"Bicubic"`, `"PDFX4:2010"`).
    Name(String),
    /// PostScript string literal `(...)` or decoded hex string `<...>`.
    Str(String),
    Array(Vec<JobValue>),
    Dict(BTreeMap<String, JobValue>),
}

impl JobValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(v) => Some(*v),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Real(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&str> {
        match self {
            Self::Name(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) | Self::Name(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// An Adobe PDF export preset (`.joboptions`).
#[derive(Clone, Debug, PartialEq)]
pub struct PdfPreset {
    pub name: String,
    pub description: String,
    pub compatibility_level: f64,
    pub standard: Standard,
    pub spreads: bool,
    pub use_document_bleed: bool,
    pub bleed_pt: [f64; 4], // top, bottom, left, right
    pub marks: Marks,
    pub registration_marks: bool,
    pub color_bars: bool,
    pub compress_pages: bool,
    pub compress_images: bool,
    pub color_image_dpi: u32,
    pub gray_image_dpi: u32,
    pub mono_image_dpi: u32,
    pub tagged: bool,
    pub bookmarks: bool,
    pub hyperlinks: bool,
    pub interactive_elements: bool,
    pub flatten_dpi: f32,
    pub color_conversion: String,
    pub output_intent_profile: Option<String>,
    /// Unrecognized Distiller dictionary entries preserved across import/export round-trips.
    pub extra: BTreeMap<String, JobValue>,
}

impl Default for PdfPreset {
    fn default() -> Self {
        Self::high_quality_print()
    }
}

impl PdfPreset {
    pub fn high_quality_print() -> Self {
        Self {
            name: "[High Quality Print]".into(),
            description: "High-quality desktop and proof printing preset (PDF 1.7).".into(),
            compatibility_level: 1.7,
            standard: Standard::None,
            spreads: false,
            use_document_bleed: false,
            bleed_pt: [0.0; 4],
            marks: Marks::NONE,
            registration_marks: false,
            color_bars: false,
            compress_pages: true,
            compress_images: true,
            color_image_dpi: 300,
            gray_image_dpi: 300,
            mono_image_dpi: 1200,
            tagged: true,
            bookmarks: true,
            hyperlinks: true,
            interactive_elements: false,
            flatten_dpi: 300.0,
            color_conversion: "LeaveColorUnchanged".into(),
            output_intent_profile: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn press_quality() -> Self {
        Self {
            name: "[Press Quality]".into(),
            description: "CMYK commercial press output preset.".into(),
            compatibility_level: 1.6,
            color_conversion: "ConvertToCMYK".into(),
            output_intent_profile: Some("Coated FOGRA39 (ISO 12647-2:2004)".into()),
            tagged: false,
            bookmarks: false,
            hyperlinks: false,
            ..Self::high_quality_print()
        }
    }

    pub fn smallest_file_size() -> Self {
        Self {
            name: "[Smallest File Size]".into(),
            description: "Compact RGB web and email distribution preset.".into(),
            compatibility_level: 1.7,
            color_image_dpi: 100,
            gray_image_dpi: 100,
            mono_image_dpi: 300,
            color_conversion: "ConvertToSRGB".into(),
            ..Self::high_quality_print()
        }
    }

    pub fn pdfx1a_2003() -> Self {
        Self {
            name: "[PDF/X-1a:2003]".into(),
            description: "ISO 15930-4 blind CMYK/spot exchange (PDF 1.4, flattened transparency).".into(),
            compatibility_level: 1.4,
            standard: Standard::PdfX1a,
            use_document_bleed: true,
            ..Self::press_quality()
        }
    }

    pub fn pdfx3_2003() -> Self {
        Self {
            name: "[PDF/X-3:2003]".into(),
            description: "ISO 15930-6 color-managed exchange (PDF 1.4, flattened transparency, ICC RGB allowed).".into(),
            compatibility_level: 1.4,
            standard: Standard::PdfX3,
            use_document_bleed: true,
            color_conversion: "LeaveColorUnchanged".into(),
            ..Self::press_quality()
        }
    }

    pub fn pdfx4_2008() -> Self {
        Self {
            name: "[PDF/X-4:2008]".into(),
            description: "ISO 15930-7 modern print exchange (PDF 1.6, live transparency, ICC color).".into(),
            compatibility_level: 1.6,
            standard: Standard::PdfX4,
            use_document_bleed: true,
            color_conversion: "LeaveColorUnchanged".into(),
            ..Self::press_quality()
        }
    }

    /// Standard built-in InDesign PDF presets.
    pub fn builtins() -> Vec<Self> {
        vec![
            Self::high_quality_print(),
            Self::press_quality(),
            Self::smallest_file_size(),
            Self::pdfx1a_2003(),
            Self::pdfx3_2003(),
            Self::pdfx4_2008(),
        ]
    }

    /// Creates a [`PdfPreset`] snapshot from [`PdfOptions`].
    pub fn from_options(name: impl Into<String>, opts: &PdfOptions) -> Self {
        let compatibility_level = match opts.standard {
            Standard::PdfX1a | Standard::PdfX3 => 1.4,
            Standard::PdfX4 => 1.6,
            Standard::None | Standard::PdfA2b => 1.7,
        };
        Self {
            name: name.into(),
            compatibility_level,
            standard: opts.standard,
            spreads: opts.spreads,
            use_document_bleed: opts.bleed,
            marks: opts.marks,
            compress_pages: opts.compress,
            compress_images: opts.compress_images,
            tagged: opts.tagged,
            interactive_elements: opts.media,
            flatten_dpi: opts.flatten_dpi,
            ..Self::high_quality_print()
        }
    }

    /// Applies this preset's settings onto `opts`.
    pub fn apply_to(&self, opts: &mut PdfOptions) {
        opts.standard = self.standard;
        opts.spreads = self.spreads;
        opts.bleed = self.use_document_bleed || self.bleed_pt.iter().any(|v| *v > 0.0);
        opts.marks = self.marks;
        opts.compress = self.compress_pages;
        opts.compress_images = self.compress_images;
        opts.tagged = self.tagged;
        opts.media = self.interactive_elements;
        opts.flatten_dpi = self.flatten_dpi;
    }
}

/// Decodes raw `.joboptions` bytes (UTF-8, UTF-16LE, UTF-16BE, or Latin-1 fallback) and parses the
/// preset dictionary.
pub fn parse_joboptions_bytes(bytes: &[u8], default_name: &str) -> std::result::Result<PdfPreset, String> {
    let text = decode_text(bytes);
    parse_joboptions(&text, default_name)
}

fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|b| char::from(*b)).collect(),
    }
}

/// Parses an Adobe `.joboptions` string into a [`PdfPreset`].
pub fn parse_joboptions(input: &str, default_name: &str) -> std::result::Result<PdfPreset, String> {
    let mut parser = PsParser::new(input.as_bytes());
    let dict = parser.parse_root_dict()?;
    Ok(preset_from_dict(dict, default_name))
}

const KNOWN_KEYS: &[&str] = &[
    "PresetName",
    "Description",
    "CompatibilityLevel",
    "CheckCompliance",
    "PDFX1aCheck",
    "PDFX3Check",
    "PDFX4Check",
    "PDFACompliance",
    "SpreadControl",
    "UseDocumentBleed",
    "BleedTop",
    "BleedBottom",
    "BleedLeft",
    "BleedRight",
    "CropMarks",
    "BleedMarks",
    "PageInformationMarks",
    "RegistrationMarks",
    "ColorBars",
    "MarkWeight",
    "PageMarksOffset",
    "BleedOffset",
    "CompressPages",
    "AutoCompress",
    "DownsampleColorImages",
    "ColorImageFilter",
    "PassThroughJPEGImages",
    "ColorImageResolution",
    "GrayImageResolution",
    "MonoImageResolution",
    "IncludeTaggedPDF",
    "IncludeBookmarks",
    "IncludeHyperlinks",
    "IncludeInteractiveElements",
    "TransparencyFlattenerPreset",
    "ColorConversionStrategy",
    "OutputIntentProfileName",
    "PDFXOutputIntentProfile",
];

fn preset_from_dict(mut dict: BTreeMap<String, JobValue>, default_name: &str) -> PdfPreset {
    let mut p = PdfPreset::high_quality_print();
    p.name = dict
        .get("PresetName")
        .and_then(JobValue::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(default_name)
        .to_string();

    if let Some(desc) = dict.get("Description") {
        match desc {
            JobValue::Str(s) => p.description = s.clone(),
            JobValue::Dict(d) => {
                if let Some(s) = d
                    .get("ENU")
                    .or_else(|| d.get("KOR"))
                    .or_else(|| d.values().next())
                    .and_then(JobValue::as_str)
                {
                    p.description = s.to_string();
                }
            }
            _ => {}
        }
    }

    if let Some(level) = dict.get("CompatibilityLevel").and_then(JobValue::as_f64) {
        p.compatibility_level = level;
    }

    // Standard resolution: CheckCompliance array first, then boolean flags.
    p.standard = Standard::None;
    if let Some(JobValue::Array(items)) = dict.get("CheckCompliance") {
        for item in items {
            if let Some(name) = item.as_str() {
                let norm: String = name
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .collect::<String>()
                    .to_ascii_lowercase();
                if norm.starts_with("pdfx1a") {
                    p.standard = Standard::PdfX1a;
                } else if norm.starts_with("pdfx3") {
                    p.standard = Standard::PdfX3;
                } else if norm.starts_with("pdfx4") {
                    p.standard = Standard::PdfX4;
                } else if norm.starts_with("pdfa") {
                    p.standard = Standard::PdfA2b;
                }
            }
        }
    }
    if p.standard == Standard::None {
        if dict.get("PDFX1aCheck").and_then(JobValue::as_bool) == Some(true) {
            p.standard = Standard::PdfX1a;
        } else if dict.get("PDFX3Check").and_then(JobValue::as_bool) == Some(true) {
            p.standard = Standard::PdfX3;
        } else if dict.get("PDFX4Check").and_then(JobValue::as_bool) == Some(true) {
            p.standard = Standard::PdfX4;
        } else if dict
            .get("PDFACompliance")
            .is_some_and(|v| v.as_bool() == Some(true) || v.as_i64().unwrap_or(0) > 0)
        {
            p.standard = Standard::PdfA2b;
        }
    }

    if let Some(b) = dict.get("SpreadControl").and_then(JobValue::as_bool) {
        p.spreads = b;
    }
    if let Some(b) = dict.get("UseDocumentBleed").and_then(JobValue::as_bool) {
        p.use_document_bleed = b;
    }
    for (idx, key) in ["BleedTop", "BleedBottom", "BleedLeft", "BleedRight"].iter().enumerate() {
        if let Some(v) = dict.get(*key).and_then(JobValue::as_f64) {
            p.bleed_pt[idx] = v.max(0.0);
        }
    }

    let mut marks = Marks::NONE;
    if let Some(b) = dict.get("CropMarks").and_then(JobValue::as_bool) {
        marks.crop = b;
    }
    if let Some(b) = dict.get("BleedMarks").and_then(JobValue::as_bool) {
        marks.bleed = b;
    }
    if let Some(b) = dict.get("PageInformationMarks").and_then(JobValue::as_bool) {
        marks.page_info = b;
    }
    if let Some(w) = dict.get("MarkWeight").and_then(JobValue::as_f64) {
        marks.weight = w.clamp(0.05, 10.0);
    }
    if let Some(off) = dict
        .get("PageMarksOffset")
        .or_else(|| dict.get("BleedOffset"))
        .and_then(JobValue::as_f64)
    {
        marks.offset = off.clamp(0.0, 72.0);
    }
    p.marks = marks;
    p.registration_marks = dict.get("RegistrationMarks").and_then(JobValue::as_bool).unwrap_or(false);
    p.color_bars = dict.get("ColorBars").and_then(JobValue::as_bool).unwrap_or(false);

    if let Some(b) = dict
        .get("CompressPages")
        .or_else(|| dict.get("AutoCompress"))
        .and_then(JobValue::as_bool)
    {
        p.compress_pages = b;
    }

    let downsample = dict.get("DownsampleColorImages").and_then(JobValue::as_bool);
    let dct = dict.get("ColorImageFilter").and_then(JobValue::as_str) == Some("DCTEncode");
    if let Some(ds) = downsample {
        p.compress_images = ds || dct;
    } else if dct {
        p.compress_images = true;
    }

    if let Some(dpi) = dict.get("ColorImageResolution").and_then(JobValue::as_i64) {
        p.color_image_dpi = dpi.clamp(36, 2400) as u32;
    }
    if let Some(dpi) = dict.get("GrayImageResolution").and_then(JobValue::as_i64) {
        p.gray_image_dpi = dpi.clamp(36, 2400) as u32;
    }
    if let Some(dpi) = dict.get("MonoImageResolution").and_then(JobValue::as_i64) {
        p.mono_image_dpi = dpi.clamp(72, 4800) as u32;
    }

    if let Some(b) = dict.get("IncludeTaggedPDF").and_then(JobValue::as_bool) {
        p.tagged = b;
    }
    if let Some(b) = dict.get("IncludeBookmarks").and_then(JobValue::as_bool) {
        p.bookmarks = b;
    }
    if let Some(b) = dict.get("IncludeHyperlinks").and_then(JobValue::as_bool) {
        p.hyperlinks = b;
    }
    if let Some(b) = dict.get("IncludeInteractiveElements").and_then(JobValue::as_bool) {
        p.interactive_elements = b;
    }

    if let Some(flat) = dict.get("TransparencyFlattenerPreset").and_then(JobValue::as_str) {
        let low = flat.to_ascii_lowercase();
        if low.contains("low") {
            p.flatten_dpi = 72.0;
        } else if low.contains("medium") {
            p.flatten_dpi = 150.0;
        } else if low.contains("high") {
            p.flatten_dpi = 300.0;
        }
    }

    if let Some(conv) = dict.get("ColorConversionStrategy").and_then(JobValue::as_str) {
        p.color_conversion = conv.to_string();
    }
    if let Some(prof) = dict
        .get("OutputIntentProfileName")
        .or_else(|| dict.get("PDFXOutputIntentProfile"))
        .and_then(JobValue::as_str)
        .filter(|s| !s.is_empty())
    {
        p.output_intent_profile = Some(prof.to_string());
    }

    for k in KNOWN_KEYS {
        dict.remove(*k);
    }
    p.extra = dict;
    p
}

/// Serializes a [`PdfPreset`] into standard Adobe `.joboptions` PostScript dictionary text.
pub fn write_joboptions(preset: &PdfPreset) -> String {
    let mut dict = preset.extra.clone();
    dict.insert("PresetName".into(), JobValue::Str(preset.name.clone()));
    if !preset.description.is_empty() {
        let mut desc = BTreeMap::new();
        desc.insert("ENU".into(), JobValue::Str(preset.description.clone()));
        dict.insert("Description".into(), JobValue::Dict(desc));
    }

    let level = match preset.standard {
        Standard::PdfX1a | Standard::PdfX3 => 1.4,
        Standard::PdfX4 => 1.6,
        _ => preset.compatibility_level,
    };
    dict.insert("CompatibilityLevel".into(), JobValue::Real(level));

    let compliance = match preset.standard {
        Standard::None => Vec::new(),
        Standard::PdfX1a => vec![JobValue::Name("PDFX1a:2003".into())],
        Standard::PdfX3 => vec![JobValue::Name("PDFX3:2003".into())],
        Standard::PdfX4 => vec![JobValue::Name("PDFX4:2010".into())],
        Standard::PdfA2b => vec![JobValue::Name("PDFA2b:2011".into())],
    };
    dict.insert("CheckCompliance".into(), JobValue::Array(compliance));
    dict.insert("PDFX1aCheck".into(), JobValue::Bool(preset.standard == Standard::PdfX1a));
    dict.insert("PDFX3Check".into(), JobValue::Bool(preset.standard == Standard::PdfX3));
    dict.insert("PDFX4Check".into(), JobValue::Bool(preset.standard == Standard::PdfX4));
    dict.insert("PDFACompliance".into(), JobValue::Bool(preset.standard == Standard::PdfA2b));

    dict.insert("SpreadControl".into(), JobValue::Bool(preset.spreads));
    dict.insert("UseDocumentBleed".into(), JobValue::Bool(preset.use_document_bleed));
    dict.insert("BleedTop".into(), JobValue::Real(preset.bleed_pt[0]));
    dict.insert("BleedBottom".into(), JobValue::Real(preset.bleed_pt[1]));
    dict.insert("BleedLeft".into(), JobValue::Real(preset.bleed_pt[2]));
    dict.insert("BleedRight".into(), JobValue::Real(preset.bleed_pt[3]));

    dict.insert("CropMarks".into(), JobValue::Bool(preset.marks.crop));
    dict.insert("BleedMarks".into(), JobValue::Bool(preset.marks.bleed));
    dict.insert("PageInformationMarks".into(), JobValue::Bool(preset.marks.page_info));
    dict.insert("RegistrationMarks".into(), JobValue::Bool(preset.registration_marks));
    dict.insert("ColorBars".into(), JobValue::Bool(preset.color_bars));
    dict.insert("MarkWeight".into(), JobValue::Real(preset.marks.weight));
    dict.insert("PageMarksOffset".into(), JobValue::Real(preset.marks.offset));

    dict.insert("CompressPages".into(), JobValue::Bool(preset.compress_pages));
    dict.insert("DownsampleColorImages".into(), JobValue::Bool(preset.compress_images));
    dict.insert(
        "ColorImageFilter".into(),
        JobValue::Name(if preset.compress_images { "DCTEncode" } else { "FlateEncode" }.into()),
    );
    dict.insert("ColorImageResolution".into(), JobValue::Int(i64::from(preset.color_image_dpi)));
    dict.insert("GrayImageResolution".into(), JobValue::Int(i64::from(preset.gray_image_dpi)));
    dict.insert("MonoImageResolution".into(), JobValue::Int(i64::from(preset.mono_image_dpi)));

    dict.insert("EmbedAllFonts".into(), JobValue::Bool(true));
    dict.insert("SubsetFonts".into(), JobValue::Bool(true));
    dict.insert("IncludeTaggedPDF".into(), JobValue::Bool(preset.tagged));
    dict.insert("IncludeBookmarks".into(), JobValue::Bool(preset.bookmarks));
    dict.insert("IncludeHyperlinks".into(), JobValue::Bool(preset.hyperlinks));
    dict.insert("IncludeInteractiveElements".into(), JobValue::Bool(preset.interactive_elements));

    let flat_name = if preset.flatten_dpi <= 100.0 {
        "LowResolution"
    } else if preset.flatten_dpi <= 200.0 {
        "MediumResolution"
    } else {
        "HighResolution"
    };
    dict.insert("TransparencyFlattenerPreset".into(), JobValue::Name(flat_name.into()));
    dict.insert("ColorConversionStrategy".into(), JobValue::Name(preset.color_conversion.clone()));
    if let Some(prof) = &preset.output_intent_profile {
        dict.insert("OutputIntentProfileName".into(), JobValue::Str(prof.clone()));
        dict.insert("PDFXOutputIntentProfile".into(), JobValue::Str(prof.clone()));
    }

    let mut out = String::from("%!PS-Adobe-3.0\n%%Creator: DesignCraft\n");
    write_value(&mut out, &JobValue::Dict(dict), 0);
    out.push_str(" setdistillerparams\n");
    out
}

fn write_value(out: &mut String, val: &JobValue, indent: usize) {
    let pad = "  ".repeat(indent);
    match val {
        JobValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JobValue::Int(i) => out.push_str(&i.to_string()),
        JobValue::Real(v) => out.push_str(&format!("{v:.6}")),
        JobValue::Name(n) => {
            out.push('/');
            out.push_str(n);
        }
        JobValue::Str(s) => {
            out.push('(');
            for ch in s.chars() {
                match ch {
                    '(' => out.push_str("\\("),
                    ')' => out.push_str("\\)"),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c => out.push(c),
                }
            }
            out.push(')');
        }
        JobValue::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
            } else {
                out.push_str("[\n");
                let child_pad = "  ".repeat(indent + 1);
                for item in items {
                    out.push_str(&child_pad);
                    write_value(out, item, indent + 1);
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push(']');
            }
        }
        JobValue::Dict(map) => {
            out.push_str("<<\n");
            let child_pad = "  ".repeat(indent + 1);
            for (k, v) in map {
                out.push_str(&child_pad);
                out.push('/');
                out.push_str(k);
                out.push(' ');
                write_value(out, v, indent + 1);
                out.push('\n');
            }
            out.push_str(&pad);
            out.push_str(">>");
        }
    }
}

struct PsParser<'a> {
    src: &'a [u8],
    pos: usize,
}

const MAX_NESTING: usize = 32;

impl<'a> PsParser<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0 }
    }

    fn skip_ws_and_comments(&mut self) {
        while let Some(&b) = self.src.get(self.pos) {
            if b.is_ascii_whitespace() || b == 0 {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(&c) = self.src.get(self.pos) {
                    self.pos += 1;
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn parse_root_dict(&mut self) -> std::result::Result<BTreeMap<String, JobValue>, String> {
        // Scan past optional PostScript prolog tokens before the first `<<`.
        while self.pos < self.src.len() {
            self.skip_ws_and_comments();
            if self.src.get(self.pos..self.pos + 2) == Some(b"<<") {
                self.pos += 2;
                return self.parse_dict_body(0);
            }
            if self.pos < self.src.len() {
                self.pos += 1;
            }
        }
        Err("no `<< ... >>` preset dictionary found in .joboptions file".into())
    }

    fn parse_dict_body(&mut self, depth: usize) -> std::result::Result<BTreeMap<String, JobValue>, String> {
        if depth > MAX_NESTING {
            return Err("maximum dictionary nesting exceeded in .joboptions".into());
        }
        let mut map = BTreeMap::new();
        loop {
            self.skip_ws_and_comments();
            if self.pos >= self.src.len() {
                return Err("unterminated `<<` dictionary in .joboptions".into());
            }
            if self.src.get(self.pos..self.pos + 2) == Some(b">>") {
                self.pos += 2;
                return Ok(map);
            }
            let key = match self.parse_value(depth + 1)? {
                JobValue::Name(k) | JobValue::Str(k) => k,
                other => return Err(format!("expected `/Key` name in .joboptions dictionary, got {other:?}")),
            };
            let val = self.parse_value(depth + 1)?;
            map.insert(key, val);
        }
    }

    fn parse_value(&mut self, depth: usize) -> std::result::Result<JobValue, String> {
        if depth > MAX_NESTING {
            return Err("maximum nesting exceeded in .joboptions".into());
        }
        self.skip_ws_and_comments();
        let Some(&b) = self.src.get(self.pos) else {
            return Err("unexpected end of .joboptions input".into());
        };
        match b {
            b'<' if self.src.get(self.pos + 1) == Some(&b'<') => {
                self.pos += 2;
                Ok(JobValue::Dict(self.parse_dict_body(depth)?))
            }
            b'<' => self.parse_hex_string(),
            b'(' => self.parse_literal_string(),
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws_and_comments();
                    if self.pos >= self.src.len() {
                        return Err("unterminated `[` array in .joboptions".into());
                    }
                    if self.src.get(self.pos) == Some(&b']') {
                        self.pos += 1;
                        return Ok(JobValue::Array(items));
                    }
                    items.push(self.parse_value(depth + 1)?);
                }
            }
            b'/' => {
                self.pos += 1;
                let start = self.pos;
                while let Some(&c) = self.src.get(self.pos) {
                    if c.is_ascii_whitespace() || b"()<>[]{}/%".contains(&c) {
                        break;
                    }
                    self.pos += 1;
                }
                let raw = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
                Ok(JobValue::Name(decode_ps_name(&raw)))
            }
            _ => {
                let start = self.pos;
                while let Some(&c) = self.src.get(self.pos) {
                    if c.is_ascii_whitespace() || b"()<>[]{}/%".contains(&c) {
                        break;
                    }
                    self.pos += 1;
                }
                let tok = String::from_utf8_lossy(&self.src[start..self.pos]);
                match tok.as_ref() {
                    "true" => Ok(JobValue::Bool(true)),
                    "false" => Ok(JobValue::Bool(false)),
                    "null" => Ok(JobValue::Name("null".into())),
                    _ => {
                        if let Ok(i) = tok.parse::<i64>() {
                            Ok(JobValue::Int(i))
                        } else if let Ok(f) = tok.parse::<f64>() {
                            Ok(JobValue::Real(f))
                        } else {
                            Ok(JobValue::Name(tok.into_owned()))
                        }
                    }
                }
            }
        }
    }

    fn parse_literal_string(&mut self) -> std::result::Result<JobValue, String> {
        self.pos += 1; // skip '('
        let mut bytes = Vec::new();
        let mut depth = 1usize;
        while let Some(&b) = self.src.get(self.pos) {
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    bytes.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(JobValue::Str(decode_ps_string_bytes(&bytes)));
                    }
                    bytes.push(b')');
                }
                b'\\' => {
                    let Some(&esc) = self.src.get(self.pos) else { break };
                    self.pos += 1;
                    match esc {
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'b' => bytes.push(0x08),
                        b'f' => bytes.push(0x0C),
                        b'(' => bytes.push(b'('),
                        b')' => bytes.push(b')'),
                        b'\\' => bytes.push(b'\\'),
                        b'\r' => {
                            if self.src.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        d if (b'0'..=b'7').contains(&d) => {
                            let mut val = u32::from(d - b'0');
                            for _ in 0..2 {
                                if let Some(&nd) = self.src.get(self.pos)
                                    && (b'0'..=b'7').contains(&nd)
                                {
                                    self.pos += 1;
                                    val = val * 8 + u32::from(nd - b'0');
                                } else {
                                    break;
                                }
                            }
                            bytes.push((val & 0xFF) as u8);
                        }
                        other => bytes.push(other),
                    }
                }
                other => bytes.push(other),
            }
        }
        Err("unterminated `(` string literal in .joboptions".into())
    }

    fn parse_hex_string(&mut self) -> std::result::Result<JobValue, String> {
        self.pos += 1; // skip '<'
        let mut nibbles = Vec::new();
        while let Some(&b) = self.src.get(self.pos) {
            self.pos += 1;
            if b == b'>' {
                if nibbles.len() % 2 == 1 {
                    nibbles.push(0);
                }
                let raw: Vec<u8> = nibbles.chunks_exact(2).map(|c| (c[0] << 4) | c[1]).collect();
                return Ok(JobValue::Str(decode_ps_string_bytes(&raw)));
            }
            if b.is_ascii_whitespace() {
                continue;
            }
            let nib = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err("invalid hex digit in `<...>` string in .joboptions".into()),
            };
            nibbles.push(nib);
        }
        Err("unterminated `<` hex string in .joboptions".into())
    }
}

fn decode_ps_string_bytes(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|b| char::from(*b)).collect(),
    }
}

fn decode_ps_name(raw: &str) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#'
            && let Some(hex) = bytes.get(i + 1..i + 3)
            && let Ok(s) = std::str::from_utf8(hex)
            && let Ok(v) = u8::from_str_radix(s, 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_distiller_joboptions_and_roundtrips_extra_keys() {
        let sample = r#"
%!PS-Adobe-3.0
<<
  /CheckCompliance [
    /PDFX3:2003
  ]
  /CompatibilityLevel 1.4
  /CropMarks true
  /BleedMarks true
  /PageInformationMarks false
  /MarkWeight 0.500000
  /PageMarksOffset 9.000000
  /UseDocumentBleed true
  /BleedTop 9.0
  /BleedBottom 9.0
  /BleedLeft 9.0
  /BleedRight 9.0
  /ColorImageResolution 300
  /DownsampleColorImages true
  /OutputIntentProfileName (Coated FOGRA39 \(ISO 12647-2:2004\))
  /Description <<
    /ENU (Commercial press PDF/X-3 preset)
    /KOR <FEFFAC78C1C4C6A9>
  >>
  /CustomVendorFlag /KeepMe
>> setdistillerparams
"#;
        let preset = parse_joboptions(sample, "Custom X3").unwrap();
        assert_eq!(preset.name, "Custom X3");
        assert_eq!(preset.standard, Standard::PdfX3);
        assert_eq!(preset.compatibility_level, 1.4);
        assert!(preset.marks.crop);
        assert!(preset.marks.bleed);
        assert!(!preset.marks.page_info);
        assert!((preset.marks.weight - 0.5).abs() < 1e-6);
        assert!((preset.marks.offset - 9.0).abs() < 1e-6);
        assert!(preset.use_document_bleed);
        assert_eq!(preset.output_intent_profile.as_deref(), Some("Coated FOGRA39 (ISO 12647-2:2004)"));
        assert_eq!(preset.extra.get("CustomVendorFlag"), Some(&JobValue::Name("KeepMe".into())));

        let mut opts = PdfOptions::default();
        preset.apply_to(&mut opts);
        assert_eq!(opts.standard, Standard::PdfX3);
        assert!(opts.bleed);
        assert!(opts.marks.crop);

        let written = write_joboptions(&preset);
        let reparsed = parse_joboptions(&written, "Fallback").unwrap();
        assert_eq!(reparsed.standard, Standard::PdfX3);
        assert_eq!(reparsed.marks, preset.marks);
        assert_eq!(reparsed.extra.get("CustomVendorFlag"), Some(&JobValue::Name("KeepMe".into())));
    }
}
