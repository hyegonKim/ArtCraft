//! Clean-room XFL package export (`DOMDocument.xml`, `<stem>.xfl`, `LIBRARY/*.xml`, media).
//!
//! Produces an uncompressed XFL package directory (or in-memory file map `(relative_path, bytes)`)
//! written strictly from the public XFL XML schema and round-tripping cleanly through
//! [`crate::import_with`] / [`crate::import_path`].

use anicraft_doc::sound::SoundEffect;
use anicraft_doc::stroke_style::Dash;
use anicraft_doc::{
    BitmapId, Cap, ColorEffect, Document, Easing, Element, ElementKind, Filter, FilterEntry,
    FilterSide, FrameScript, GradientStop, Join, Keyframe, Language, Layer, LayerKind, LibraryItem,
    LoopMode, Paint, Shape, SoundFormat, SoundId, SoundSync, Spread, Stroke, Symbol,
    SymbolId, SymbolKind, TextAlign, TextBlock, TextKind, Timeline, Tween, TweenKind,
};
use anicraft_geom::{Affine, Path, PathCmd, Vec2};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::XflError;

const TWIPS: f32 = 20.0;
const GRADIENT_HALF: f32 = 819.2;

/// Summary of items exported or approximated while writing an XFL package.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub approximated: BTreeMap<String, usize>,
    pub files_written: usize,
}

impl ExportReport {
    fn note(&mut self, what: &str) {
        *self.approximated.entry(what.to_string()).or_default() += 1;
    }

    pub fn summary(&self) -> String {
        if self.approximated.is_empty() {
            return String::new();
        }
        let items: Vec<String> = self.approximated.iter().map(|(k, n)| format!("{n} {k}")).collect();
        format!("Approximated in XFL: {}.", items.join(", "))
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

fn sanitize_stem(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c })
        .take(96)
        .collect();
    let trimmed = cleaned.trim_matches(['.', ' ']);
    if trimmed.is_empty() { fallback.to_string() } else { trimmed.to_string() }
}

fn folder_prefix(doc: &Document, item: LibraryItem) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut cur = doc.library_tree.placed.get(&item).copied();
    let mut guard = 0usize;
    while let Some(fid) = cur {
        if guard >= 64 {
            break;
        }
        guard += 1;
        let Some(f) = doc.library_tree.folders.iter().find(|f| f.id == fid) else { break };
        parts.push(f.name.as_str());
        cur = f.parent;
    }
    parts.reverse();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    }
}

fn folder_full_path(doc: &Document, folder_id: anicraft_doc::FolderId) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut cur = Some(folder_id);
    let mut guard = 0usize;
    while let Some(fid) = cur {
        if guard >= 64 {
            break;
        }
        guard += 1;
        let Some(f) = doc.library_tree.folders.iter().find(|f| f.id == fid) else { break };
        parts.push(f.name.as_str());
        cur = f.parent;
    }
    parts.reverse();
    parts.join("/")
}

fn write_matrix(xml: &mut String, indent: &str, m: &Affine) {
    if *m == Affine::IDENTITY {
        return;
    }
    let [a, b, c, d, tx, ty] = m.m;
    let _ = writeln!(
        xml,
        "{indent}<matrix><Matrix a=\"{a:.6}\" b=\"{b:.6}\" c=\"{c:.6}\" d=\"{d:.6}\" tx=\"{tx:.4}\" ty=\"{ty:.4}\"/></matrix>"
    );
}

fn split_cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> ([Vec2; 4], [Vec2; 4]) {
    let p01 = p0.lerp(p1, 0.5);
    let p12 = p1.lerp(p2, 0.5);
    let p23 = p2.lerp(p3, 0.5);
    let p012 = p01.lerp(p12, 0.5);
    let p123 = p12.lerp(p23, 0.5);
    let mid = p012.lerp(p123, 0.5);
    ([p0, p01, p012, mid], [mid, p123, p23, p3])
}

fn cubic_to_quads(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, depth: u8, out: &mut Vec<(Vec2, Vec2)>) {
    // Degree-reduction midpoint control point q = (3*(p1 + p2) - (p0 + p3)) / 4;
    // error bound is ||(p3 - 3*p2 + 3*p1 - p0)|| * sqrt(3)/36.
    let d = (p3 - p2 * 3.0) + (p1 * 3.0 - p0);
    let err = d.length() * (1.732_050_8 / 36.0);
    if err <= 0.05 || depth >= 4 {
        let q = ((p1 + p2) * 3.0 - (p0 + p3)) * 0.25;
        out.push((q, p3));
    } else {
        let (left, right) = split_cubic(p0, p1, p2, p3);
        cubic_to_quads(left[0], left[1], left[2], left[3], depth + 1, out);
        cubic_to_quads(right[0], right[1], right[2], right[3], depth + 1, out);
    }
}

fn twip(v: f32) -> i64 {
    if v.is_finite() { (f64::from(v) * f64::from(TWIPS)).round() as i64 } else { 0 }
}

fn encode_xfl_edges(path: &Path, close_open_for_fill: bool) -> String {
    let mut s = String::new();
    let mut start = Vec2::ZERO;
    let mut cur = Vec2::ZERO;
    let mut in_sub = false;
    let mut quads = Vec::new();

    for cmd in &path.cmds {
        match *cmd {
            PathCmd::MoveTo(p) => {
                if close_open_for_fill && in_sub && (cur - start).length() > 1.0e-3 {
                    let _ = write!(s, "|{} {} ", twip(start.x), twip(start.y));
                }
                let _ = write!(s, "!{} {} ", twip(p.x), twip(p.y));
                start = p;
                cur = p;
                in_sub = true;
            }
            PathCmd::LineTo(p) => {
                if !in_sub {
                    let _ = write!(s, "!{} {} ", twip(cur.x), twip(cur.y));
                    start = cur;
                    in_sub = true;
                }
                let _ = write!(s, "|{} {} ", twip(p.x), twip(p.y));
                cur = p;
            }
            PathCmd::QuadTo(c, p) => {
                if !in_sub {
                    let _ = write!(s, "!{} {} ", twip(cur.x), twip(cur.y));
                    start = cur;
                    in_sub = true;
                }
                let _ = write!(s, "[{} {} {} {} ", twip(c.x), twip(c.y), twip(p.x), twip(p.y));
                cur = p;
            }
            PathCmd::CubicTo(c1, c2, p) => {
                if !in_sub {
                    let _ = write!(s, "!{} {} ", twip(cur.x), twip(cur.y));
                    start = cur;
                    in_sub = true;
                }
                quads.clear();
                cubic_to_quads(cur, c1, c2, p, 0, &mut quads);
                for &(qc, qp) in &quads {
                    let _ = write!(s, "[{} {} {} {} ", twip(qc.x), twip(qc.y), twip(qp.x), twip(qp.y));
                }
                cur = p;
            }
            PathCmd::Close => {
                if in_sub && (cur - start).length() > 1.0e-3 {
                    let _ = write!(s, "|{} {} ", twip(start.x), twip(start.y));
                }
                cur = start;
                in_sub = false;
            }
        }
    }
    if close_open_for_fill && in_sub && (cur - start).length() > 1.0e-3 {
        let _ = write!(s, "|{} {} ", twip(start.x), twip(start.y));
    }
    s.trim().to_string()
}

struct Exporter<'a> {
    doc: &'a Document,
    symbol_names: HashMap<SymbolId, String>,
    bitmap_names: HashMap<BitmapId, (String, String)>,
    sound_names: HashMap<SoundId, (String, String)>,
    report: ExportReport,
}

impl<'a> Exporter<'a> {
    fn new(doc: &'a Document) -> Self {
        let mut symbol_names = HashMap::new();
        for sym in &doc.library {
            let full = format!("{}{}", folder_prefix(doc, LibraryItem::Symbol(sym.id)), sym.name);
            symbol_names.insert(sym.id, full);
        }
        let mut bitmap_names = HashMap::new();
        for bmp in &doc.bitmaps {
            let full = format!("{}{}", folder_prefix(doc, LibraryItem::Bitmap(bmp.id)), bmp.name);
            let href = format!("bitmaps/bmp_{}.png", bmp.id.0);
            bitmap_names.insert(bmp.id, (full, href));
        }
        let mut sound_names = HashMap::new();
        for snd in &doc.sounds {
            let full = format!("{}{}", folder_prefix(doc, LibraryItem::Sound(snd.id)), snd.name);
            let ext = match snd.format {
                SoundFormat::Wav => "wav",
                SoundFormat::Ogg => "ogg",
                SoundFormat::Mp3 => "mp3",
            };
            let href = format!("sounds/snd_{}.{ext}", snd.id.0);
            sound_names.insert(snd.id, (full, href));
        }
        Self { doc, symbol_names, bitmap_names, sound_names, report: ExportReport::default() }
    }

    fn write_stops(xml: &mut String, indent: &str, stops: &[GradientStop]) {
        for stop in stops {
            let c = stop.color.sanitized();
            let _ = writeln!(
                xml,
                "{indent}<GradientEntry color=\"{}\" alpha=\"{:.4}\" ratio=\"{:.4}\"/>",
                c.to_hex(),
                c.a,
                stop.offset.clamp(0.0, 1.0)
            );
        }
    }

    fn write_paint(&mut self, xml: &mut String, indent: &str, paint: &Paint) {
        match paint {
            Paint::Solid(c) => {
                let s = c.sanitized();
                let _ = writeln!(xml, "{indent}<SolidColor color=\"{}\" alpha=\"{:.4}\"/>", s.to_hex(), s.a);
            }
            Paint::Linear { stops, spread, linear_rgb, .. } => {
                let spread_attr = if *spread == Spread::Pad { String::new() } else { format!(" spreadMethod=\"{}\"", spread.key()) };
                let interp_attr = if *linear_rgb { " interpolationMethod=\"linearRGB\"" } else { "" };
                let _ = writeln!(xml, "{indent}<LinearGradient{spread_attr}{interp_attr}>");
                if let Some(frame) = paint.frame(None) {
                    let inv = Affine::scale(1.0 / GRADIENT_HALF, 1.0 / GRADIENT_HALF);
                    write_matrix(xml, &format!("{indent}  "), &frame.mul(&inv));
                }
                Self::write_stops(xml, &format!("{indent}  "), stops);
                let _ = writeln!(xml, "{indent}</LinearGradient>");
            }
            Paint::Radial { stops, spread, linear_rgb, focal, .. } => {
                let spread_attr = if *spread == Spread::Pad { String::new() } else { format!(" spreadMethod=\"{}\"", spread.key()) };
                let interp_attr = if *linear_rgb { " interpolationMethod=\"linearRGB\"" } else { "" };
                let focal_attr = if focal.abs() > 1.0e-5 { format!(" focalPointRatio=\"{focal:.4}\"") } else { String::new() };
                let _ = writeln!(xml, "{indent}<RadialGradient{spread_attr}{interp_attr}{focal_attr}>");
                if let Some(frame) = paint.frame(None) {
                    let inv = Affine::scale(1.0 / GRADIENT_HALF, 1.0 / GRADIENT_HALF);
                    write_matrix(xml, &format!("{indent}  "), &frame.mul(&inv));
                }
                Self::write_stops(xml, &format!("{indent}  "), stops);
                let _ = writeln!(xml, "{indent}</RadialGradient>");
            }
            Paint::Bitmap { bitmap, repeat, matrix } => {
                if let Some((name, _)) = self.bitmap_names.get(bitmap) {
                    let clip = if *repeat { "" } else { " bitmapIsClipped=\"true\"" };
                    let _ = writeln!(xml, "{indent}<BitmapFill bitmapPath=\"{}\"{clip}>", xml_escape(name));
                    let [a, b, c, d, tx, ty] = matrix.m;
                    let scaled = Affine { m: [a * TWIPS, b * TWIPS, c * TWIPS, d * TWIPS, tx, ty] };
                    write_matrix(xml, &format!("{indent}  "), &scaled);
                    let _ = writeln!(xml, "{indent}</BitmapFill>");
                } else {
                    self.report.note("bitmap fills referencing missing bitmaps");
                    let _ = writeln!(xml, "{indent}<SolidColor color=\"#808080\" alpha=\"1\"/>");
                }
            }
        }
    }

    fn write_stroke(&mut self, xml: &mut String, indent: &str, stroke: &Stroke) {
        if stroke.profile.is_some() {
            self.report.note("variable-width stroke profiles (exported uniform)");
        }
        let caps = match stroke.style.cap {
            Cap::Butt => " caps=\"none\"",
            Cap::Square => " caps=\"square\"",
            Cap::Round => "",
        };
        let joints = match stroke.style.join {
            Join::Miter => " joints=\"miter\"",
            Join::Bevel => " joints=\"bevel\"",
            Join::Round => "",
        };
        let miter = if (stroke.style.miter_limit - anicraft_doc::stroke_style::DEFAULT_MITER_LIMIT).abs() > 1.0e-3 {
            format!(" miterLimit=\"{:.3}\"", stroke.style.miter_limit)
        } else {
            String::new()
        };
        let (tag, dash_attrs) = match &stroke.style.dash {
            Some(Dash::Dashed { lengths, .. }) => {
                let on = lengths.first().copied().unwrap_or(4.0);
                let off = lengths.get(1).copied().unwrap_or(on);
                ("DashedStroke", format!(" dash1=\"{on:.3}\" dash2=\"{off:.3}\""))
            }
            Some(Dash::Dotted { spacing }) => ("DottedStroke", format!(" dotSpace=\"{spacing:.3}\"")),
            None => ("SolidStroke", String::new()),
        };
        let c = stroke.color.sanitized();
        let _ = writeln!(
            xml,
            "{indent}<{tag} scaleMode=\"normal\" weight=\"{:.3}\"{caps}{joints}{miter}{dash_attrs}>",
            stroke.width.clamp(0.05, 1000.0)
        );
        let _ = writeln!(xml, "{indent}  <fill><SolidColor color=\"{}\" alpha=\"{:.4}\"/></fill>", c.to_hex(), c.a);
        let _ = writeln!(xml, "{indent}</{tag}>");
    }

    fn write_shape(&mut self, xml: &mut String, indent: &str, el: &Element, sh: &Shape, in_group: bool) {
        if sh.warp.is_some() {
            self.report.note("asset warp meshes (exported as deformed outlines)");
        }
        let fill_edges = sh.fill.as_ref().map(|_| encode_xfl_edges(&sh.path, true)).unwrap_or_default();
        let stroke_edges = sh.stroke.as_ref().map(|_| encode_xfl_edges(&sh.path, false)).unwrap_or_default();
        if fill_edges.is_empty() && stroke_edges.is_empty() {
            return;
        }
        let drawing_obj = if !in_group && !sh.merge { " isDrawingObject=\"true\"" } else { "" };
        let _ = writeln!(xml, "{indent}<DOMShape{drawing_obj}>");
        write_matrix(xml, &format!("{indent}  "), &el.transform);
        if let Some(fill) = &sh.fill {
            let _ = writeln!(xml, "{indent}  <fills><FillStyle index=\"1\">");
            self.write_paint(xml, &format!("{indent}    "), fill);
            let _ = writeln!(xml, "{indent}  </FillStyle></fills>");
        }
        if let Some(stroke) = &sh.stroke {
            let _ = writeln!(xml, "{indent}  <strokes><StrokeStyle index=\"1\">");
            self.write_stroke(xml, &format!("{indent}    "), stroke);
            let _ = writeln!(xml, "{indent}  </StrokeStyle></strokes>");
        }
        let _ = writeln!(xml, "{indent}  <edges>");
        if !fill_edges.is_empty() && sh.fill.is_some() {
            let _ = writeln!(xml, "{indent}    <Edge fillStyle1=\"1\" edges=\"{}\"/>", xml_escape(&fill_edges));
        }
        if !stroke_edges.is_empty() && sh.stroke.is_some() {
            let _ = writeln!(xml, "{indent}    <Edge strokeStyle=\"1\" edges=\"{}\"/>", xml_escape(&stroke_edges));
        }
        let _ = writeln!(xml, "{indent}  </edges>");
        let _ = writeln!(xml, "{indent}</DOMShape>");
    }

    fn write_color_effect(xml: &mut String, indent: &str, fx: &ColorEffect) {
        match fx {
            ColorEffect::None => {}
            ColorEffect::Alpha(a) => {
                let _ = writeln!(xml, "{indent}<color><Color alphaMultiplier=\"{a:.4}\"/></color>");
            }
            ColorEffect::Brightness(b) => {
                let _ = writeln!(xml, "{indent}<color><Color brightness=\"{b:.4}\"/></color>");
            }
            ColorEffect::Tint { color, amount } => {
                let _ = writeln!(
                    xml,
                    "{indent}<color><Color tintMultiplier=\"{amount:.4}\" tintColor=\"{}\"/></color>",
                    color.to_hex()
                );
            }
            ColorEffect::Advanced(t) => {
                let [rm, gm, bm, am] = t.mul;
                let [ra, ga, ba, aa] = t.add.map(|v| v * 255.0);
                let _ = writeln!(
                    xml,
                    "{indent}<color><Color redMultiplier=\"{rm:.4}\" greenMultiplier=\"{gm:.4}\" blueMultiplier=\"{bm:.4}\" alphaMultiplier=\"{am:.4}\" redOffset=\"{ra:.2}\" greenOffset=\"{ga:.2}\" blueOffset=\"{ba:.2}\" alphaOffset=\"{aa:.2}\"/></color>"
                );
            }
        }
    }

    fn write_filters(xml: &mut String, indent: &str, filters: &[FilterEntry]) {
        if filters.is_empty() {
            return;
        }
        let side_str = |s: FilterSide| match s {
            FilterSide::Inner => "inner",
            FilterSide::Outer => "outer",
            FilterSide::Full => "full",
        };
        let _ = writeln!(xml, "{indent}<filters>");
        for entry in filters {
            let en = if entry.enabled { "" } else { " enabled=\"false\"" };
            match &entry.filter {
                Filter::Blur { blur_x, blur_y, quality } => {
                    let _ = writeln!(xml, "{indent}  <BlurFilter blurX=\"{blur_x:.3}\" blurY=\"{blur_y:.3}\" quality=\"{quality}\"{en}/>");
                }
                Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, inner, knockout, hide_object } => {
                    let c = color.sanitized();
                    let _ = writeln!(
                        xml,
                        "{indent}  <DropShadowFilter blurX=\"{blur_x:.3}\" blurY=\"{blur_y:.3}\" strength=\"{strength:.3}\" quality=\"{quality}\" angle=\"{angle:.3}\" distance=\"{distance:.3}\" color=\"{}\" alpha=\"{:.4}\" inner=\"{inner}\" knockout=\"{knockout}\" hideObject=\"{hide_object}\"{en}/>",
                        c.to_hex(),
                        c.a
                    );
                }
                Filter::Glow { blur_x, blur_y, strength, quality, color, inner, knockout } => {
                    let c = color.sanitized();
                    let _ = writeln!(
                        xml,
                        "{indent}  <GlowFilter blurX=\"{blur_x:.3}\" blurY=\"{blur_y:.3}\" strength=\"{strength:.3}\" quality=\"{quality}\" color=\"{}\" alpha=\"{:.4}\" inner=\"{inner}\" knockout=\"{knockout}\"{en}/>",
                        c.to_hex(),
                        c.a
                    );
                }
                Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                    let _ = writeln!(
                        xml,
                        "{indent}  <AdjustColorFilter brightness=\"{brightness:.3}\" contrast=\"{contrast:.3}\" saturation=\"{saturation:.3}\" hue=\"{hue:.3}\"{en}/>"
                    );
                }
                Filter::Bevel { blur_x, blur_y, strength, quality, angle, distance, shadow_color, highlight_color, side, knockout } => {
                    let sc = shadow_color.sanitized();
                    let hc = highlight_color.sanitized();
                    let _ = writeln!(
                        xml,
                        "{indent}  <BevelFilter blurX=\"{blur_x:.3}\" blurY=\"{blur_y:.3}\" strength=\"{strength:.3}\" quality=\"{quality}\" angle=\"{angle:.3}\" distance=\"{distance:.3}\" shadowColor=\"{}\" shadowAlpha=\"{:.4}\" highlightColor=\"{}\" highlightAlpha=\"{:.4}\" type=\"{}\" knockout=\"{knockout}\"{en}/>",
                        sc.to_hex(),
                        sc.a,
                        hc.to_hex(),
                        hc.a,
                        side_str(*side)
                    );
                }
                Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, stops, side, knockout }
                | Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, stops, side, knockout } => {
                    let tag = if matches!(entry.filter, Filter::GradientGlow { .. }) { "GradientGlowFilter" } else { "GradientBevelFilter" };
                    let _ = writeln!(
                        xml,
                        "{indent}  <{tag} blurX=\"{blur_x:.3}\" blurY=\"{blur_y:.3}\" strength=\"{strength:.3}\" quality=\"{quality}\" angle=\"{angle:.3}\" distance=\"{distance:.3}\" type=\"{}\" knockout=\"{knockout}\"{en}>",
                        side_str(*side)
                    );
                    Self::write_stops(xml, &format!("{indent}    "), stops);
                    let _ = writeln!(xml, "{indent}  </{tag}>");
                }
            }
        }
        let _ = writeln!(xml, "{indent}</filters>");
    }

    fn write_text(&mut self, xml: &mut String, indent: &str, el: &Element, tb: &TextBlock) {
        if tb.on_path.is_some() {
            self.report.note("text on a path (exported as outlined vector shape)");
            let sh = Shape {
                path: tb.outline.clone(),
                fill: Some(Paint::Solid(tb.color)),
                stroke: None,
                fill_rule: anicraft_geom::FillRule::NonZero,
                merge: false,
                warp: None,
            };
            self.write_shape(xml, indent, el, &sh, false);
            return;
        }
        let tag = match tb.kind() {
            TextKind::Static => "DOMStaticText",
            TextKind::Dynamic => "DOMDynamicText",
            TextKind::Input => "DOMInputText",
        };
        let name_attr = if el.name.is_empty() { String::new() } else { format!(" name=\"{}\"", xml_escape(&el.name)) };
        let w = tb.width.unwrap_or(tb.box_width).max(4.0);
        let h = tb.box_height.max(4.0);
        let align = match tb.align {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        };
        let c = tb.color.sanitized();
        let _ = writeln!(xml, "{indent}<{tag}{name_attr} width=\"{w:.2}\" height=\"{h:.2}\">");
        write_matrix(xml, &format!("{indent}  "), &el.transform);
        let _ = writeln!(xml, "{indent}  <textRuns><DOMTextRun>");
        let _ = writeln!(xml, "{indent}    <characters>{}</characters>", xml_escape(&tb.text));
        let _ = writeln!(
            xml,
            "{indent}    <textAttrs><DOMTextAttrs alignment=\"{align}\" size=\"{:.2}\" face=\"{}\" bold=\"{}\" italic=\"{}\" fillColor=\"{}\" alpha=\"{:.4}\" letterSpacing=\"{:.2}\" lineSpacing=\"{:.2}\"/></textAttrs>",
            tb.size,
            xml_escape(&tb.font),
            tb.bold,
            tb.italic,
            c.to_hex(),
            c.a,
            tb.letter_spacing,
            tb.line_spacing
        );
        let _ = writeln!(xml, "{indent}  </DOMTextRun></textRuns>");
        let _ = writeln!(xml, "{indent}</{tag}>");
    }

    fn write_elements(&mut self, xml: &mut String, indent: &str, els: &[std::sync::Arc<Element>], in_group: bool, depth: usize) {
        if depth > anicraft_doc::MAX_GROUP_DEPTH {
            return;
        }
        for el in els {
            match &el.kind {
                ElementKind::Shape(sh) => self.write_shape(xml, indent, el, sh, in_group),
                ElementKind::Symbol { symbol, loop_mode, first_frame, blend, filters, events } => {
                    if !events.is_empty() {
                        self.report.note("instance button/clip pointer actions");
                    }
                    let Some(sym_name) = self.symbol_names.get(symbol).cloned() else { continue };
                    let name_attr = if el.name.is_empty() { String::new() } else { format!(" name=\"{}\"", xml_escape(&el.name)) };
                    let loop_str = match loop_mode {
                        LoopMode::Loop => "loop",
                        LoopMode::PlayOnce => "play once",
                        LoopMode::SingleFrame => "single frame",
                    };
                    let first_attr = if *first_frame > 0 { format!(" firstFrame=\"{first_frame}\"") } else { String::new() };
                    let blend_attr = if blend.is_normal() { String::new() } else { format!(" blendMode=\"{}\"", blend.key()) };
                    let _ = writeln!(
                        xml,
                        "{indent}<DOMSymbolInstance libraryItemName=\"{}\"{name_attr} loop=\"{loop_str}\"{first_attr}{blend_attr}>",
                        xml_escape(&sym_name)
                    );
                    write_matrix(xml, &format!("{indent}  "), &el.transform);
                    Self::write_color_effect(xml, &format!("{indent}  "), &el.color);
                    Self::write_filters(xml, &format!("{indent}  "), filters);
                    let _ = writeln!(xml, "{indent}</DOMSymbolInstance>");
                }
                ElementKind::Text(tb) => self.write_text(xml, indent, el, tb),
                ElementKind::Camera => {}
                ElementKind::Group { .. } => {
                    let members = el.kind.group_members();
                    if !members.is_empty() {
                        let _ = writeln!(xml, "{indent}<DOMGroup>");
                        write_matrix(xml, &format!("{indent}  "), &el.transform);
                        let _ = writeln!(xml, "{indent}  <members>");
                        self.write_elements(xml, &format!("{indent}    "), &members, true, depth + 1);
                        let _ = writeln!(xml, "{indent}  </members>");
                        let _ = writeln!(xml, "{indent}</DOMGroup>");
                    }
                }
            }
        }
    }

    fn write_custom_ease_points(xml: &mut String, indent: &str, target: &str, ease: &Easing) {
        let curve = ease.to_custom();
        let segs = curve.points().len().saturating_sub(1);
        if segs == 0 {
            return;
        }
        let _ = writeln!(xml, "{indent}<CustomEase target=\"{target}\">");
        for i in 0..segs {
            if let Some([p0, p1, p2, p3]) = curve.segment(i) {
                if i == 0 {
                    let _ = writeln!(xml, "{indent}  <Point x=\"{:.6}\" y=\"{:.6}\"/>", p0.x, p0.y);
                }
                let _ = writeln!(xml, "{indent}  <Point x=\"{:.6}\" y=\"{:.6}\"/>", p1.x, p1.y);
                let _ = writeln!(xml, "{indent}  <Point x=\"{:.6}\" y=\"{:.6}\"/>", p2.x, p2.y);
                let _ = writeln!(xml, "{indent}  <Point x=\"{:.6}\" y=\"{:.6}\"/>", p3.x, p3.y);
            }
        }
        let _ = writeln!(xml, "{indent}</CustomEase>");
    }

    fn write_tween_attrs(tween: &Tween) -> (String, bool) {
        let mut attrs = String::new();
        let t_type = match tween.kind {
            TweenKind::Classic => "motion",
            TweenKind::Shape => "shape",
        };
        let _ = write!(attrs, " tweenType=\"{t_type}\"");
        if tween.orient {
            attrs.push_str(" motionTweenOrientToPath=\"true\"");
        }
        let has_groups = !tween.groups.is_empty();
        match (&tween.easing, has_groups) {
            (Easing::Linear, false) => (attrs, false),
            (Easing::Classic(acc), false) => {
                let _ = write!(attrs, " acceleration=\"{acc:.2}\"");
                (attrs, false)
            }
            (_, false) => {
                attrs.push_str(" hasCustomEase=\"true\" useSingleEaseCurve=\"true\"");
                (attrs, true)
            }
            (_, true) => {
                attrs.push_str(" hasCustomEase=\"true\" useSingleEaseCurve=\"false\"");
                (attrs, true)
            }
        }
    }

    fn write_keyframe(&mut self, xml: &mut String, indent: &str, kf: &Keyframe) {
        if kf.motion.is_some() {
            self.report.note("property-curve motion tweens (exported at start keyframe)");
        }
        let mut attrs = format!(" index=\"{}\" duration=\"{}\"", kf.start, kf.duration.max(1));
        if !kf.label.is_empty() {
            let _ = write!(attrs, " name=\"{}\"", xml_escape(&kf.label));
        }
        let mut emit_custom_ease = false;
        if let Some(tween) = &kf.tween {
            let (ta, custom) = Self::write_tween_attrs(tween);
            attrs.push_str(&ta);
            emit_custom_ease = custom;
        }
        if let Some(fs) = &kf.sound
            && let Some((snd_name, _)) = self.sound_names.get(&fs.sound)
        {
            let sync = match fs.sync {
                SoundSync::Event => "event",
                SoundSync::Start => "start",
                SoundSync::Stop => "stop",
                SoundSync::Stream => "stream",
            };
            let loop_mode = if fs.looping { "loop" } else { "repeat" };
            let fx = match &fs.effect {
                SoundEffect::None => "none",
                SoundEffect::LeftChannel => "left channel",
                SoundEffect::RightChannel => "right channel",
                SoundEffect::FadeLeftToRight => "fade left to right",
                SoundEffect::FadeRightToLeft => "fade right to left",
                SoundEffect::FadeIn => "fade in",
                SoundEffect::FadeOut => "fade out",
                SoundEffect::Custom(_) => "custom",
            };
            let _ = write!(
                attrs,
                " soundName=\"{}\" soundSync=\"{sync}\" soundLoopMode=\"{loop_mode}\" soundLoop=\"{}\" soundEffect=\"{fx}\"",
                xml_escape(snd_name),
                fs.repeat.max(1)
            );
        }

        let _ = writeln!(xml, "{indent}<DOMFrame{attrs}>");
        if emit_custom_ease && let Some(tween) = &kf.tween {
            let _ = writeln!(xml, "{indent}  <tweens>");
            Self::write_custom_ease_points(xml, &format!("{indent}    "), "all", &tween.easing);
            for (group, ge) in &tween.groups {
                Self::write_custom_ease_points(xml, &format!("{indent}    "), group.key(), ge);
            }
            let _ = writeln!(xml, "{indent}  </tweens>");
        }
        if !kf.hints.is_empty() {
            let _ = writeln!(xml, "{indent}  <shapeHints>");
            for (idx, h) in kf.hints.iter().enumerate() {
                let _ = writeln!(
                    xml,
                    "{indent}    <ShapeHint index=\"{idx}\" startX=\"{:.3}\" startY=\"{:.3}\" endX=\"{:.3}\" endY=\"{:.3}\"/>",
                    h.start.x, h.start.y, h.end.x, h.end.y
                );
            }
            let _ = writeln!(xml, "{indent}  </shapeHints>");
        }
        if let Some(script) = &kf.script {
            match script {
                FrameScript::Source(src) => {
                    let _ = writeln!(xml, "{indent}  <Actionscript><script>{}</script></Actionscript>", xml_escape(&src.text));
                }
                FrameScript::Actions(_) | FrameScript::Plugin(_) => {
                    self.report.note("non-AS frame scripts");
                }
            }
        }
        if let Some(fs) = &kf.sound
            && let SoundEffect::Custom(pts) = &fs.effect
            && !pts.is_empty()
        {
            let _ = writeln!(xml, "{indent}  <SoundEnvelope>");
            for pt in pts {
                let mark44 = (pt.at.max(0.0) * 44_100.0).round() as u64;
                let l0 = (pt.left.clamp(0.0, 1.0) * 32_768.0).round() as u32;
                let l1 = (pt.right.clamp(0.0, 1.0) * 32_768.0).round() as u32;
                let _ = writeln!(xml, "{indent}    <SoundEnvelopePoint mark44=\"{mark44}\" level0=\"{l0}\" level1=\"{l1}\"/>");
            }
            let _ = writeln!(xml, "{indent}  </SoundEnvelope>");
        }
        let _ = writeln!(xml, "{indent}  <elements>");
        self.write_elements(xml, &format!("{indent}    "), &kf.elements, false, 0);
        let _ = writeln!(xml, "{indent}  </elements>");
        let _ = writeln!(xml, "{indent}</DOMFrame>");
    }

    fn write_layer(&mut self, xml: &mut String, indent: &str, layer: &Layer, index_by_id: &HashMap<anicraft_doc::LayerId, usize>) {
        if layer.kind == LayerKind::Camera {
            self.report.note("camera layers (omitted in XFL)");
            return;
        }
        if layer.kind == LayerKind::Armature {
            self.report.note("armature bone rigs (exported as normal layers)");
        }
        let ltype = match layer.kind {
            LayerKind::Folder => " layerType=\"folder\"",
            LayerKind::Guide => " layerType=\"guide\"",
            LayerKind::Mask => " layerType=\"mask\"",
            LayerKind::Normal | LayerKind::Armature | LayerKind::Camera => "",
        };
        let parent_attr = layer
            .parent
            .and_then(|pid| index_by_id.get(&pid))
            .map(|idx| format!(" parentLayerIndex=\"{idx}\""))
            .unwrap_or_default();
        let vis = if layer.visible { "" } else { " visible=\"false\"" };
        let lock = if layer.locked { " locked=\"true\"" } else { "" };
        let outl = if layer.outline { " outline=\"true\"" } else { "" };
        let h_mult = if layer.height_rows > 1 { format!(" heightMultiplier=\"{}\"", layer.height_rows) } else { String::new() };
        let _ = writeln!(
            xml,
            "{indent}<DOMLayer name=\"{}\" color=\"{}\"{ltype}{parent_attr}{vis}{lock}{outl}{h_mult}>",
            xml_escape(&layer.name),
            layer.color.to_hex()
        );
        if layer.kind != LayerKind::Folder {
            let _ = writeln!(xml, "{indent}  <frames>");
            for kf in &layer.keyframes {
                self.write_keyframe(xml, &format!("{indent}    "), kf);
            }
            let _ = writeln!(xml, "{indent}  </frames>");
        }
        let _ = writeln!(xml, "{indent}</DOMLayer>");
    }

    fn write_timeline(&mut self, xml: &mut String, indent: &str, name: &str, tl: &Timeline) {
        let mut index_by_id = HashMap::new();
        let mut exported_idx = 0usize;
        for l in &tl.layers {
            if l.kind != LayerKind::Camera {
                index_by_id.insert(l.id, exported_idx);
                exported_idx += 1;
            } else {
                self.report.note("camera layers (omitted in XFL)");
            }
        }
        let _ = writeln!(xml, "{indent}<DOMTimeline name=\"{}\">", xml_escape(name));
        let _ = writeln!(xml, "{indent}  <layers>");
        for l in &tl.layers {
            if l.kind != LayerKind::Camera {
                self.write_layer(xml, &format!("{indent}    "), l, &index_by_id);
            }
        }
        let _ = writeln!(xml, "{indent}  </layers>");
        let _ = writeln!(xml, "{indent}</DOMTimeline>");
    }

    fn symbol_xml(&mut self, sym: &Symbol) -> String {
        let full_name = self.symbol_names.get(&sym.id).cloned().unwrap_or_else(|| sym.name.clone());
        let stype = match sym.kind {
            SymbolKind::Graphic => " symbolType=\"graphic\"",
            SymbolKind::Button => " symbolType=\"button\"",
            SymbolKind::MovieClip => "",
        };
        let mut extra = String::new();
        if let Some(link) = &sym.linkage {
            let _ = write!(extra, " linkageExportForAS=\"true\" linkageClassName=\"{}\"", xml_escape(&link.class));
            if !link.base.is_empty() {
                let _ = write!(extra, " linkageBaseClass=\"{}\"", xml_escape(&link.base));
            }
        }
        if let Some(g) = &sym.scale_grid {
            let _ = write!(
                extra,
                " scaleGridLeft=\"{:.3}\" scaleGridRight=\"{:.3}\" scaleGridTop=\"{:.3}\" scaleGridBottom=\"{:.3}\"",
                g.left, g.right, g.top, g.bottom
            );
        }
        let mut xml = String::new();
        let _ = writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        let _ = writeln!(
            xml,
            "<DOMSymbolItem xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://ns.adobe.com/xfl/2008/\" name=\"{}\"{stype}{extra}>",
            xml_escape(&full_name)
        );
        let _ = writeln!(xml, "  <timeline>");
        self.write_timeline(&mut xml, "    ", &sym.name, &sym.timeline);
        let _ = writeln!(xml, "  </timeline>");
        let _ = writeln!(xml, "</DOMSymbolItem>");
        xml
    }

    fn dom_document_xml(&mut self) -> String {
        let mut xml = String::new();
        let _ = writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        let _ = writeln!(
            xml,
            "<DOMDocument xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://ns.adobe.com/xfl/2008/\" width=\"{}\" height=\"{}\" frameRate=\"{:.3}\" backgroundColor=\"{}\" currentTimeline=\"1\" xflVersion=\"2.97\">",
            self.doc.stage.width,
            self.doc.stage.height,
            self.doc.stage.fps,
            self.doc.stage.background.to_hex()
        );
        if !self.doc.library_tree.folders.is_empty() {
            let _ = writeln!(xml, "  <folders>");
            for f in &self.doc.library_tree.folders {
                let path = folder_full_path(self.doc, f.id);
                if !path.is_empty() {
                    let _ = writeln!(xml, "    <DOMFolderItem name=\"{}\"/>", xml_escape(&path));
                }
            }
            let _ = writeln!(xml, "  </folders>");
        }
        if !self.doc.bitmaps.is_empty() || !self.doc.sounds.is_empty() {
            let _ = writeln!(xml, "  <media>");
            for bmp in &self.doc.bitmaps {
                if let Some((full, href)) = self.bitmap_names.get(&bmp.id) {
                    let smooth = if bmp.smoothing { "true" } else { "false" };
                    let _ = writeln!(
                        xml,
                        "    <DOMBitmapItem name=\"{}\" href=\"{}\" allowSmoothing=\"{smooth}\"/>",
                        xml_escape(full),
                        xml_escape(href)
                    );
                }
            }
            for snd in &self.doc.sounds {
                if let Some((full, href)) = self.sound_names.get(&snd.id) {
                    let _ = writeln!(
                        xml,
                        "    <DOMSoundItem name=\"{}\" href=\"{}\"/>",
                        xml_escape(full),
                        xml_escape(href)
                    );
                }
            }
            let _ = writeln!(xml, "  </media>");
        }
        if !self.doc.library.is_empty() {
            let _ = writeln!(xml, "  <symbols>");
            for sym in &self.doc.library {
                let _ = writeln!(xml, "    <Include href=\"sym_{}.xml\"/>", sym.id.0);
            }
            let _ = writeln!(xml, "  </symbols>");
        }
        let _ = writeln!(xml, "  <timelines>");
        for scene in &self.doc.scenes {
            self.write_timeline(&mut xml, "    ", &scene.name, &scene.timeline);
        }
        let _ = writeln!(xml, "  </timelines>");
        let _ = writeln!(xml, "</DOMDocument>");
        xml
    }

    fn detect_script_language(&self) -> Language {
        for scene in &self.doc.scenes {
            for l in &scene.timeline.layers {
                for kf in &l.keyframes {
                    if let Some(FrameScript::Source(src)) = &kf.script {
                        return src.language;
                    }
                }
            }
        }
        Language::ActionScript3
    }
}

/// Builds a complete XFL package in memory as `(relative_path, bytes)` pairs.
pub fn export_package(doc: &Document, stem: &str) -> (Vec<(String, Vec<u8>)>, ExportReport) {
    let clean_stem = sanitize_stem(stem, "movie");
    let mut ex = Exporter::new(doc);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    files.push((format!("{clean_stem}.xfl"), b"PROXY-CS5\n".to_vec()));
    for sym in &doc.library {
        let s_xml = ex.symbol_xml(sym);
        files.push((format!("LIBRARY/sym_{}.xml", sym.id.0), s_xml.into_bytes()));
    }
    for bmp in &doc.bitmaps {
        if let Some((_, href)) = ex.bitmap_names.get(&bmp.id) {
            files.push((format!("LIBRARY/{href}"), (*bmp.png).clone()));
        }
    }
    for snd in &doc.sounds {
        if let Some((_, href)) = ex.sound_names.get(&snd.id) {
            files.push((format!("LIBRARY/{href}"), (*snd.data).clone()));
        }
    }
    let dom = ex.dom_document_xml();
    files.push(("DOMDocument.xml".to_string(), dom.into_bytes()));

    let lang = ex.detect_script_language();
    if !doc.document_class.is_empty() || lang != Language::ActionScript3 {
        let as_ver = match lang {
            Language::ActionScript1or2 => "2",
            Language::ActionScript3 | Language::Other => "3",
        };
        let publish = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<flash_profile version=\"1.0\" name=\"Default\" current=\"true\">\n  <PublishFlashProperties>\n    <ActionScriptVersion>{as_ver}</ActionScriptVersion>\n    <DocumentClass>{}</DocumentClass>\n  </PublishFlashProperties>\n</flash_profile>\n",
            xml_escape(&doc.document_class)
        );
        files.push(("PublishSettings.xml".to_string(), publish.into_bytes()));
    }

    ex.report.files_written = files.len();
    (files, ex.report)
}

/// Packs `(relative_path, bytes)` entries into a standard Deflate ZIP archive (`.fla`).
fn pack_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    let mut count = 0u16;
    for (rel, raw) in files {
        if count == u16::MAX {
            break;
        }
        let name = rel.replace('\\', "/");
        let name_bytes = name.as_bytes();
        if name_bytes.len() > usize::from(u16::MAX) || raw.len() > u32::MAX as usize || out.len() > u32::MAX as usize {
            continue;
        }
        let deflated = miniz_oxide::deflate::compress_to_vec(raw, 6);
        let (method, payload): (u16, &[u8]) = if deflated.len() < raw.len() {
            (8, &deflated)
        } else {
            (0, raw.as_slice())
        };
        let crc = crate::zip::crc32(raw);
        let local_offset = out.len() as u32;

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x0021u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(payload);

        cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0x0800u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0x0021u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&local_offset.to_le_bytes());
        cd.extend_from_slice(name_bytes);
        count = count.saturating_add(1);
    }
    let cd_offset = out.len() as u32;
    let cd_len = cd.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Exports `doc` as a compressed `.fla` ZIP archive in memory.
pub fn export_fla(doc: &Document, stem: &str) -> (Vec<u8>, ExportReport) {
    let (files, report) = export_package(doc, stem);
    (pack_zip(&files), report)
}

/// Writes an XFL package directly to disk at `dir`.
pub fn export_path(doc: &Document, dir: &std::path::Path, stem: Option<&str>) -> Result<ExportReport, XflError> {
    let default_stem = dir
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("movie");
    let (files, report) = export_package(doc, stem.unwrap_or(default_stem));
    std::fs::create_dir_all(dir)?;
    for (rel, bytes) in files {
        let full = dir.join(&rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(full, bytes)?;
    }
    Ok(report)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod export_tests {
    use super::*;
    use crate::import_with;
    use anicraft_doc::{Rgba, sample};
    use std::sync::Arc;

    #[test]
    fn sample_demo_round_trips_through_xfl_package() {
        let orig = sample::demo();
        let (files, export_report) = export_package(&orig, "demo");
        assert!(export_report.files_written >= 3);
        let map: HashMap<String, Vec<u8>> = files.into_iter().collect();
        assert_eq!(map.get("demo.xfl").map(Vec::as_slice), Some(&b"PROXY-CS5\n"[..]));

        let (reimported, import_report) = import_with(&|p| Ok(map.get(p).cloned())).unwrap();
        assert!(import_report.skipped.is_empty(), "unexpected skipped items: {:?}", import_report.skipped);
        assert_eq!(reimported.stage.width, orig.stage.width);
        assert_eq!(reimported.stage.height, orig.stage.height);
        assert!((reimported.stage.fps - orig.stage.fps).abs() < 1.0e-3);
        assert_eq!(reimported.scenes.len(), orig.scenes.len());
        assert_eq!(reimported.library.len(), orig.library.len());
    }

    #[test]
    fn sample_demo_round_trips_through_fla_zip() {
        let orig = sample::demo();
        let (fla_bytes, export_report) = export_fla(&orig, "demo");
        assert!(export_report.files_written >= 3);
        let (reimported, import_report) = crate::import_fla(&fla_bytes).unwrap();
        assert!(import_report.skipped.is_empty(), "unexpected skipped items: {:?}", import_report.skipped);
        assert_eq!(reimported.stage.width, orig.stage.width);
        assert_eq!(reimported.stage.height, orig.stage.height);
        assert_eq!(reimported.scenes.len(), orig.scenes.len());
        assert_eq!(reimported.library.len(), orig.library.len());
    }

    #[test]
    fn gradients_cubics_symbols_and_sounds_round_trip() {
        let mut doc = Document::default();
        doc.stage = anicraft_doc::Stage { width: 640, height: 360, fps: 24.0, background: Rgba::hex(0x102030) };

        // Add a WAV sound
        let wav_bytes = anicraft_doc::audio::wav::encode(24_000, 1, &[0.0, 0.4, -0.4, 0.1], anicraft_doc::audio::wav::Bits::Int16);
        let snd_id = SoundId(doc.alloc_id());
        doc.sounds.push(Arc::new(anicraft_doc::Sound::from_data(snd_id, "beep.wav", wav_bytes).unwrap()));

        // Add a Graphic symbol with a cubic curve + linear gradient
        let sym_id = SymbolId(doc.alloc_id());
        let mut sym_layer = Layer::new(anicraft_doc::LayerId(doc.alloc_id()), "Art");
        let cubic_path = Path {
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
                PathCmd::CubicTo(Vec2::new(30.0, -20.0), Vec2::new(60.0, 40.0), Vec2::new(90.0, 10.0)),
                PathCmd::LineTo(Vec2::new(90.0, 50.0)),
                PathCmd::LineTo(Vec2::new(0.0, 50.0)),
                PathCmd::Close,
            ],
        };
        let grad = Paint::linear(
            Vec2::new(0.0, 0.0),
            Vec2::new(90.0, 50.0),
            vec![
                GradientStop { offset: 0.0, color: Rgba::hex(0xff3366) },
                GradientStop { offset: 1.0, color: Rgba::hex(0x33ccff) },
            ],
        );
        sym_layer.keyframe_mut(0).unwrap().elements.push(Arc::new(Element {
            id: anicraft_doc::ElementId(doc.alloc_id()),
            name: String::new(),
            transform: Affine::IDENTITY,
            color: ColorEffect::None,
            kind: ElementKind::Shape(Shape::filled(cubic_path, grad).with_stroke(Rgba::WHITE, 2.0)),
        }));
        doc.library.push(Arc::new(Symbol {
            id: sym_id,
            name: "Badge".into(),
            kind: SymbolKind::Graphic,
            linkage: None,
            scale_grid: None,
            scale_grid_kept: None,
            timeline: Timeline { layers: vec![Arc::new(sym_layer)] },
        }));

        // Place symbol instance + sound on Scene 1
        let inst_id = anicraft_doc::ElementId(doc.alloc_id());
        let scene = Arc::make_mut(&mut doc.scenes[0]);
        let kf = Arc::make_mut(&mut Arc::make_mut(&mut scene.timeline.layers[0]).keyframes[0]);
        kf.duration = 6;
        kf.sound = Some(anicraft_doc::FrameSound::new(snd_id));
        kf.elements.push(Arc::new(Element {
            id: inst_id,
            name: String::new(),
            transform: Affine::translate(120.0, 80.0),
            color: ColorEffect::Alpha(0.85),
            kind: ElementKind::Symbol {
                symbol: sym_id,
                loop_mode: LoopMode::SingleFrame,
                first_frame: 0,
                blend: anicraft_doc::BlendMode::Normal,
                filters: Vec::new(),
                events: Vec::new(),
            },
        }));

        let (files, _) = export_package(&doc, "roundtrip");
        let map: HashMap<String, Vec<u8>> = files.into_iter().collect();
        let (reimported, rep) = import_with(&|p| Ok(map.get(p).cloned())).unwrap();
        assert!(rep.skipped.is_empty(), "unexpected skipped: {:?}", rep.skipped);
        assert_eq!(reimported.sounds.len(), 1);
        assert_eq!(reimported.library.len(), 1);
        assert_eq!(reimported.stage.background, Rgba::hex(0x102030));
    }
}
