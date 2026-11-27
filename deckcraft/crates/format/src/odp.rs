//! OpenDocument Presentation (`.odp`, OASIS ODF 1.2) import and export.
//!
//! An `.odp` archive is a ZIP whose first entry is an uncompressed `mimetype` file containing
//! [`ODP_MIMETYPE`], accompanied by `META-INF/manifest.xml`, `meta.xml`, `styles.xml`,
//! `content.xml`, and embedded `Pictures/*` media streams.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use deckcraft_model::style::{ColorRef, Dash, Fill, Line, PictureFill};
use deckcraft_model::table::{Cell, Row, Table};
use deckcraft_model::text::{Action, Align, Anchor, Bullet, Hyperlink, ParaProps, Paragraph, Run, RunKind, RunProps, Strike, TextBody};
use deckcraft_model::{
    Background, Geom, LayoutId, MediaId, PhType, Placeholder, Presentation, Rgba, Shape, ShapeId, ShapeKind, Size, Slide, SlideId, Transition, Xfrm,
    defaults, resolve,
};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{FormatError, MAX_ENTRY, MAX_MEDIA_ITEMS, MAX_TOTAL_MEDIA, Result, repair};

/// OASIS OpenDocument Presentation MIME type.
pub const ODP_MIMETYPE: &str = "application/vnd.oasis.opendocument.presentation";
/// Template variant MIME type accepted on import.
pub const OTP_MIMETYPE: &str = "application/vnd.oasis.opendocument.presentation-template";

/// Does `bytes` look like an OpenDocument Presentation (`.odp` / `.otp`) archive?
pub fn sniff_odp(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK") {
        return false;
    }
    if [ODP_MIMETYPE, OTP_MIMETYPE].iter().any(|m| bytes.windows(m.len()).take(220).any(|w| w == m.as_bytes())) {
        return true;
    }
    let Ok(mut z) = zip::ZipArchive::new(Cursor::new(bytes)) else { return false };
    if let Ok(mut f) = z.by_name("mimetype") {
        let mut s = String::new();
        if f.by_ref().take(128).read_to_string(&mut s).is_ok() {
            let t = s.trim();
            if t == ODP_MIMETYPE || t == OTP_MIMETYPE {
                return true;
            }
        }
    }
    if let Ok(mut f) = z.by_name("content.xml") {
        let mut head = Vec::with_capacity(4096);
        if f.by_ref().take(4096).read_to_end(&mut head).is_ok() {
            let text = String::from_utf8_lossy(&head);
            if text.contains("office:presentation") {
                return true;
            }
        }
    }
    false
}

fn read_zip_entry(z: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, max: u64) -> Result<Option<Vec<u8>>> {
    let mut f = match z.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(FormatError::Damaged(format!("{name}: {e}"))),
    };
    if f.size() > max {
        return Err(FormatError::Damaged(format!("{name} exceeds size limit")));
    }
    let cap = usize::try_from(f.size().min(16 << 20)).unwrap_or(0);
    let mut buf = Vec::with_capacity(cap);
    f.by_ref().take(max.saturating_add(1)).read_to_end(&mut buf).map_err(|e| FormatError::Damaged(format!("{name}: {e}")))?;
    if buf.len() as u64 > max {
        return Err(FormatError::Damaged(format!("{name} exceeds size limit")));
    }
    Ok(Some(buf))
}

#[derive(Clone, Debug, Default)]
struct OdpPageStyle {
    bg_color: Option<Rgba>,
    hidden: bool,
    transition: Option<Transition>,
}

#[derive(Clone, Debug, Default)]
struct OdpGraphicStyle {
    fill_none: bool,
    fill_color: Option<Rgba>,
    fill_opacity: Option<f64>,
    stroke_none: bool,
    stroke_dashed: bool,
    stroke_color: Option<Rgba>,
    stroke_width: Option<f64>,
    anchor: Option<Anchor>,
}

#[derive(Clone, Debug, Default)]
struct OdpStyleCatalog {
    page_size: Option<Size>,
    master_bg: Option<Rgba>,
    pages: HashMap<String, OdpPageStyle>,
    graphics: HashMap<String, OdpGraphicStyle>,
    paras: HashMap<String, ParaProps>,
    texts: HashMap<String, RunProps>,
    list_bullets: HashMap<String, Vec<Bullet>>,
}

fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().rposition(|b| *b == b':') {
        Some(i) => raw.get(i + 1..).unwrap_or(raw),
        None => raw,
    }
}

fn attr_str(e: &BytesStart<'_>, wanted_local: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if local_name(a.key.as_ref()) == wanted_local {
            if let Ok(raw) = std::str::from_utf8(a.value.as_ref()) {
                return Some(quick_xml::escape::unescape(raw).map(|c| c.into_owned()).unwrap_or_else(|_| raw.to_string()));
            }
        }
    }
    None
}

fn parse_length_pt(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let split = t
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+' || *c == 'e' || *c == 'E'))
        .map(|(i, _)| i)
        .unwrap_or(t.len());
    let val: f64 = t.get(..split)?.trim().parse().ok()?;
    if !val.is_finite() {
        return None;
    }
    let unit = t.get(split..).unwrap_or("").trim().to_ascii_lowercase();
    let pt = match unit.as_str() {
        "" | "pt" => val,
        "in" => val * 72.0,
        "cm" => val * 72.0 / 2.54,
        "mm" => val * 72.0 / 25.4,
        "px" => val * 0.75,
        "pc" | "pica" => val * 12.0,
        _ => return None,
    };
    pt.is_finite().then_some(pt)
}

fn parse_hex_color(s: &str) -> Option<Rgba> {
    let h = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if h.len() == 6 {
        let r = u8::from_str_radix(h.get(0..2)?, 16).ok()?;
        let g = u8::from_str_radix(h.get(2..4)?, 16).ok()?;
        let b = u8::from_str_radix(h.get(4..6)?, 16).ok()?;
        Some(Rgba::rgb(r, g, b))
    } else if h.len() == 8 {
        let a = u8::from_str_radix(h.get(0..2)?, 16).ok()?;
        let r = u8::from_str_radix(h.get(2..4)?, 16).ok()?;
        let g = u8::from_str_radix(h.get(4..6)?, 16).ok()?;
        let b = u8::from_str_radix(h.get(6..8)?, 16).ok()?;
        Some(Rgba::rgba(r, g, b, a))
    } else {
        None
    }
}

fn parse_percent(s: &str) -> Option<f64> {
    let p = s.trim().strip_suffix('%')?.trim().parse::<f64>().ok()?;
    p.is_finite().then_some((p / 100.0).clamp(0.0, 1.0))
}

fn parse_iso_duration_ms(s: &str) -> Option<u32> {
    let t = s.trim();
    if let Some(sec) = t.strip_suffix("ms").and_then(|v| v.trim().parse::<f64>().ok()) {
        return sec.is_finite().then_some(sec.clamp(0.0, 3_600_000.0).round() as u32);
    }
    if let Some(sec) = t.strip_suffix('s').filter(|v| !v.starts_with('P')).and_then(|v| v.trim().parse::<f64>().ok()) {
        return sec.is_finite().then_some((sec * 1000.0).clamp(0.0, 3_600_000.0).round() as u32);
    }
    let body = t.strip_prefix("PT").or_else(|| t.strip_prefix("P0DT"))?;
    let mut total_sec = 0.0;
    let mut num = String::new();
    for ch in body.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else {
            let v: f64 = num.parse().unwrap_or(0.0);
            num.clear();
            match ch {
                'H' | 'h' => total_sec += v * 3600.0,
                'M' | 'm' => total_sec += v * 60.0,
                'S' | 's' => total_sec += v,
                _ => {}
            }
        }
    }
    (total_sec > 0.0 && total_sec.is_finite()).then_some((total_sec * 1000.0).clamp(0.0, 3_600_000.0).round() as u32)
}

fn parse_odp_transition(e: &BytesStart<'_>) -> Option<Transition> {
    let smil_type = attr_str(e, b"type");
    let smil_sub = attr_str(e, b"subtype").unwrap_or_default();
    let dir_rev = attr_str(e, b"direction").is_some_and(|d| d.eq_ignore_ascii_case("reverse"));
    let pres_style = attr_str(e, b"transition-style");
    let trans_type = attr_str(e, b"transition-type");
    let speed = attr_str(e, b"transition-speed");
    let dur_ms = attr_str(e, b"dur").and_then(|d| parse_iso_duration_ms(&d)).unwrap_or_else(|| match speed.as_deref() {
        Some("fast") => 500,
        Some("slow") => 1500,
        _ => 900,
    });
    let advance_after_ms = attr_str(e, b"duration").and_then(|d| parse_iso_duration_ms(&d));
    let advance_on_click = !matches!(trans_type.as_deref(), Some("automatic"));

    let (kind, option): (&str, &str) = if let Some(st) = smil_type.as_deref() {
        match st {
            "fade" => ("fade", if smil_sub == "fadeOverColor" { "throughBlack" } else { "smoothly" }),
            "pushWipe" => (
                "push",
                match (smil_sub.as_str(), dir_rev) {
                    ("fromLeft", false) | ("fromRight", true) => "r",
                    ("fromRight", false) | ("fromLeft", true) => "l",
                    ("fromTop", false) | ("fromBottom", true) => "d",
                    _ => "u",
                },
            ),
            "barWipe" => (
                "wipe",
                match (smil_sub.as_str(), dir_rev) {
                    ("leftToRight", false) | ("leftToRight", true) if !dir_rev => "r",
                    ("leftToRight", true) => "l",
                    ("topToBottom", false) => "d",
                    ("topToBottom", true) => "u",
                    _ => "r",
                },
            ),
            "barnDoorWipe" => (
                "split",
                match (smil_sub.as_str(), dir_rev) {
                    ("vertical", false) => "vertOut",
                    ("vertical", true) => "vertIn",
                    ("horizontal", false) => "horzOut",
                    _ => "horzIn",
                },
            ),
            "slideWipe" => {
                if dir_rev {
                    (
                        "uncover",
                        match smil_sub.as_str() {
                            "fromLeft" => "r",
                            "fromRight" => "l",
                            "fromTop" => "d",
                            _ => "u",
                        },
                    )
                } else {
                    (
                        "cover",
                        match smil_sub.as_str() {
                            "fromLeft" => "r",
                            "fromRight" => "l",
                            "fromTop" => "d",
                            _ => "u",
                        },
                    )
                }
            }
            "irisWipe" | "ellipseWipe" => ("zoom", if dir_rev { "out" } else { "in" }),
            "dissolve" => ("dissolve", ""),
            "checkerBoardWipe" => ("checker", if smil_sub.contains("Down") { "vert" } else { "horz" }),
            "blindsWipe" => ("blinds", if smil_sub == "horizontal" { "horz" } else { "vert" }),
            "clockWipe" => ("clock", "clockwise"),
            "morph" => ("morph", "objects"),
            "cut" => ("cut", ""),
            "none" => ("none", ""),
            _ => ("fade", "smoothly"),
        }
    } else if let Some(ps) = pres_style.as_deref() {
        match ps {
            "none" => ("none", ""),
            "fade-from-black" => ("fade", "throughBlack"),
            "fade-from-left" | "fade-from-right" | "fade-from-top" | "fade-from-bottom" | "crossfade" => ("fade", "smoothly"),
            "move-from-left" => ("push", "r"),
            "move-from-right" => ("push", "l"),
            "move-from-top" => ("push", "d"),
            "move-from-bottom" => ("push", "u"),
            "roll-from-left" => ("wipe", "r"),
            "roll-from-right" => ("wipe", "l"),
            "roll-from-top" => ("wipe", "d"),
            "roll-from-bottom" => ("wipe", "u"),
            "open-vertical" => ("split", "vertOut"),
            "close-vertical" => ("split", "vertIn"),
            "open-horizontal" => ("split", "horzOut"),
            "close-horizontal" => ("split", "horzIn"),
            "uncover-to-left" => ("uncover", "l"),
            "uncover-to-right" => ("uncover", "r"),
            "uncover-to-top" => ("uncover", "u"),
            "uncover-to-bottom" => ("uncover", "d"),
            "dissolve" => ("dissolve", ""),
            _ => ("fade", "smoothly"),
        }
    } else if advance_after_ms.is_some() || !advance_on_click {
        ("none", "")
    } else {
        return None;
    };

    Some(Transition {
        kind: kind.into(),
        option: option.into(),
        duration_ms: if kind == "none" || kind == "cut" { 0 } else { dur_ms },
        advance_on_click,
        advance_after_ms,
        ..Default::default()
    })
}

fn parse_meta_xml(xml: &[u8], pres: &mut Presentation) {
    let mut r = Reader::from_reader(xml);
    r.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut cur_tag: Option<Vec<u8>> = None;
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                cur_tag = Some(local_name(e.name().as_ref()).to_vec());
            }
            Ok(Event::Text(t)) => {
                if let (Some(tag), Ok(val)) = (&cur_tag, t.decode()) {
                    let s = quick_xml::escape::unescape(&val).map(|c| c.into_owned()).unwrap_or_else(|_| val.into_owned());
                    match tag.as_slice() {
                        b"title" => pres.props.title = s,
                        b"subject" => pres.props.subject = s,
                        b"description" => pres.props.comments = s,
                        b"initial-creator" if pres.props.author.is_empty() => pres.props.author = s,
                        b"creator" => pres.props.author = s,
                        b"keyword" => {
                            if !pres.props.keywords.is_empty() {
                                pres.props.keywords.push_str(", ");
                            }
                            pres.props.keywords.push_str(&s);
                        }
                        b"creation-date" => pres.props.created = s,
                        b"date" => pres.props.modified = s,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => cur_tag = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_styles_into(xml: &[u8], cat: &mut OdpStyleCatalog) {
    let mut r = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut cur_style_name: Option<String> = None;
    let mut cur_style_family: Option<String> = None;
    let mut cur_list_name: Option<String> = None;
    let mut master_bg_style: Option<String> = None;

    let mut handle_tag = |e: &BytesStart<'_>,
                          is_start: bool,
                          cur_style_name: &mut Option<String>,
                          cur_style_family: &mut Option<String>,
                          cur_list_name: &mut Option<String>| {
        let qname = e.name();
        let ln = local_name(qname.as_ref());
        match ln {
            b"page-layout-properties" => {
                if let (Some(w), Some(h)) =
                    (attr_str(e, b"page-width").and_then(|s| parse_length_pt(&s)), attr_str(e, b"page-height").and_then(|s| parse_length_pt(&s)))
                    && w > 10.0
                    && h > 10.0
                    && cat.page_size.is_none()
                {
                    cat.page_size = Some(Size::new(w, h));
                }
            }
            b"style" if is_start => {
                *cur_style_name = attr_str(e, b"name");
                *cur_style_family = attr_str(e, b"family");
            }
            b"master-page" => {
                if master_bg_style.is_none() {
                    master_bg_style = attr_str(e, b"style-name");
                }
            }
            b"drawing-page-properties" => {
                if let Some(name) = cur_style_name.as_ref() {
                    let entry = cat.pages.entry(name.clone()).or_default();
                    if attr_str(e, b"fill").as_deref() != Some("none")
                        && let Some(c) = attr_str(e, b"fill-color").and_then(|s| parse_hex_color(&s))
                    {
                        entry.bg_color = Some(c);
                    }
                    if attr_str(e, b"visibility").as_deref() == Some("hidden") {
                        entry.hidden = true;
                    }
                    if let Some(tr) = parse_odp_transition(e) {
                        entry.transition = Some(tr);
                    }
                }
            }
            b"graphic-properties" => {
                if let Some(name) = cur_style_name.as_ref() {
                    let entry = cat.graphics.entry(name.clone()).or_default();
                    match attr_str(e, b"fill").as_deref() {
                        Some("none") => entry.fill_none = true,
                        Some("solid") | None => {
                            if let Some(c) = attr_str(e, b"fill-color").and_then(|s| parse_hex_color(&s)) {
                                entry.fill_color = Some(c);
                            }
                        }
                        _ => {}
                    }
                    if let Some(op) = attr_str(e, b"opacity").and_then(|s| parse_percent(&s)) {
                        entry.fill_opacity = Some(op);
                    }
                    match attr_str(e, b"stroke").as_deref() {
                        Some("none") => entry.stroke_none = true,
                        Some("dash") => {
                            entry.stroke_dashed = true;
                            if let Some(c) = attr_str(e, b"stroke-color").and_then(|s| parse_hex_color(&s)) {
                                entry.stroke_color = Some(c);
                            }
                        }
                        Some("solid") | None => {
                            if let Some(c) = attr_str(e, b"stroke-color").and_then(|s| parse_hex_color(&s)) {
                                entry.stroke_color = Some(c);
                            }
                        }
                        _ => {}
                    }
                    if let Some(w) = attr_str(e, b"stroke-width").and_then(|s| parse_length_pt(&s)) {
                        entry.stroke_width = Some(w);
                    }
                    if let Some(va) = attr_str(e, b"textarea-vertical-align") {
                        entry.anchor = match va.as_str() {
                            "top" => Some(Anchor::Top),
                            "middle" => Some(Anchor::Middle),
                            "bottom" => Some(Anchor::Bottom),
                            "justify" => Some(Anchor::Justified),
                            _ => None,
                        };
                    }
                }
            }
            b"paragraph-properties" => {
                if let Some(name) = cur_style_name.as_ref() {
                    let entry = cat.paras.entry(name.clone()).or_default();
                    if let Some(al) = attr_str(e, b"text-align") {
                        entry.align = match al.as_str() {
                            "start" | "left" => Some(Align::Left),
                            "center" => Some(Align::Center),
                            "end" | "right" => Some(Align::Right),
                            "justify" => Some(Align::Justify),
                            _ => None,
                        };
                    }
                    if let Some(ml) = attr_str(e, b"margin-left").and_then(|s| parse_length_pt(&s)) {
                        entry.margin_left = Some(ml.max(0.0));
                    }
                }
            }
            b"text-properties" => {
                if let Some(name) = cur_style_name.as_ref() {
                    let entry = cat.texts.entry(name.clone()).or_default();
                    if let Some(f) = attr_str(e, b"font-name").or_else(|| attr_str(e, b"font-family")) {
                        let clean = f.trim_matches(['\'', '"']).trim().to_string();
                        if !clean.is_empty() {
                            entry.font = Some(clean);
                        }
                    }
                    if let Some(sz) = attr_str(e, b"font-size").and_then(|s| parse_length_pt(&s)) {
                        entry.size = Some(sz.clamp(1.0, 4000.0));
                    }
                    if let Some(fw) = attr_str(e, b"font-weight") {
                        entry.bold = Some(fw == "bold" || fw.parse::<u32>().is_ok_and(|n| n >= 600));
                    }
                    if let Some(fs) = attr_str(e, b"font-style") {
                        entry.italic = Some(fs == "italic" || fs == "oblique");
                    }
                    if let Some(ul) = attr_str(e, b"text-underline-style") {
                        entry.underline = Some(if ul == "none" { "none".into() } else { "sng".into() });
                    }
                    if let Some(st) = attr_str(e, b"text-line-through-style") {
                        entry.strike = Some(if st == "none" { Strike::None } else { Strike::Single });
                    }
                    if let Some(c) = attr_str(e, b"color").and_then(|s| parse_hex_color(&s)) {
                        entry.fill = Some(Fill::solid(ColorRef::rgb(c)));
                    }
                }
            }
            b"list-style" if is_start => {
                *cur_list_name = attr_str(e, b"name");
            }
            b"list-level-style-bullet" => {
                if let Some(ln_name) = cur_list_name.as_ref() {
                    let ch = attr_str(e, b"bullet-char").unwrap_or_else(|| "•".into());
                    let lvl = attr_str(e, b"level").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1).min(8);
                    let bullets = cat.list_bullets.entry(ln_name.clone()).or_default();
                    if bullets.len() <= lvl {
                        bullets.resize(lvl + 1, Bullet::Char { char: "•".into() });
                    }
                    bullets[lvl] = Bullet::Char { char: ch };
                }
            }
            b"list-level-style-number" => {
                if let Some(ln_name) = cur_list_name.as_ref() {
                    let lvl = attr_str(e, b"level").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1).min(8);
                    let bullets = cat.list_bullets.entry(ln_name.clone()).or_default();
                    if bullets.len() <= lvl {
                        bullets.resize(lvl + 1, Bullet::AutoNum { scheme: "arabicPeriod".into(), start_at: 1 });
                    }
                    bullets[lvl] = Bullet::AutoNum { scheme: "arabicPeriod".into(), start_at: 1 };
                }
            }
            _ => {}
        }
    };

    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                handle_tag(&e, true, &mut cur_style_name, &mut cur_style_family, &mut cur_list_name);
            }
            Ok(Event::Empty(e)) => {
                let is_style = local_name(e.name().as_ref()) == b"style";
                handle_tag(&e, is_style, &mut cur_style_name, &mut cur_style_family, &mut cur_list_name);
                if is_style {
                    cur_style_name = None;
                    cur_style_family = None;
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                b"style" => {
                    cur_style_name = None;
                    cur_style_family = None;
                }
                b"list-style" => {
                    cur_list_name = None;
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if cat.master_bg.is_none()
        && let Some(ms) = master_bg_style
        && let Some(ps) = cat.pages.get(&ms)
    {
        cat.master_bg = ps.bg_color;
    }
}

fn parse_xfrm(e: &BytesStart<'_>) -> Xfrm {
    let mut x = attr_str(e, b"x").and_then(|s| parse_length_pt(&s));
    let mut y = attr_str(e, b"y").and_then(|s| parse_length_pt(&s));
    let mut w = attr_str(e, b"width").and_then(|s| parse_length_pt(&s)).unwrap_or(144.0);
    let mut h = attr_str(e, b"height").and_then(|s| parse_length_pt(&s)).unwrap_or(72.0);
    if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
        attr_str(e, b"x1").and_then(|s| parse_length_pt(&s)),
        attr_str(e, b"y1").and_then(|s| parse_length_pt(&s)),
        attr_str(e, b"x2").and_then(|s| parse_length_pt(&s)),
        attr_str(e, b"y2").and_then(|s| parse_length_pt(&s)),
    ) {
        x = Some(x1.min(x2));
        y = Some(y1.min(y2));
        w = (x2 - x1).abs();
        h = (y2 - y1).abs();
    }
    let mut rot = 0.0;
    let mut trans: Option<(f64, f64)> = None;
    let mut rot_rad: Option<f64> = None;
    if let Some(tf) = attr_str(e, b"transform") {
        let mut rest = tf.as_str();
        while let Some(open) = rest.find('(') {
            let fn_name = rest.get(..open).unwrap_or("").trim().split_whitespace().last().unwrap_or("");
            let after = rest.get(open + 1..).unwrap_or("");
            let Some(close) = after.find(')') else { break };
            let args = after.get(..close).unwrap_or("");
            let parts: Vec<&str> = args.split(|c: char| c == ',' || c.is_ascii_whitespace()).filter(|p| !p.is_empty()).collect();
            match fn_name {
                "rotate" => {
                    if let Some(a) = parts.first().and_then(|s| s.parse::<f64>().ok()).filter(|v| v.is_finite()) {
                        rot_rad = Some(a);
                        rot = (-a).to_degrees().rem_euclid(360.0);
                    }
                }
                "translate" => {
                    if let (Some(tx), Some(ty)) = (parts.first().and_then(|s| parse_length_pt(s)), parts.get(1).and_then(|s| parse_length_pt(s))) {
                        trans = Some((tx, ty));
                    }
                }
                _ => {}
            }
            rest = after.get(close + 1..).unwrap_or("");
        }
    }
    let (fx, fy) = match (x, y, trans, rot_rad) {
        (Some(px), Some(py), _, _) => (px, py),
        (None, None, Some((tx, ty)), Some(alpha)) => {
            let theta = -alpha;
            let cx = tx + (w * 0.5) * theta.cos() - (h * 0.5) * theta.sin();
            let cy = ty + (w * 0.5) * theta.sin() + (h * 0.5) * theta.cos();
            (cx - w * 0.5, cy - h * 0.5)
        }
        (_, _, Some((tx, ty)), None) => (x.unwrap_or(tx), y.unwrap_or(ty)),
        _ => (x.unwrap_or(0.0), y.unwrap_or(0.0)),
    };
    Xfrm { x: fx, y: fy, w: w.max(0.0), h: h.max(0.0), rot, flip_h: false, flip_v: false }
}

fn map_odp_geom_to_preset(odp_type: &str) -> &'static str {
    match odp_type.trim() {
        "rectangle" | "non-primitive" | "" => "rect",
        "round-rectangle" => "roundRect",
        "ellipse" | "circle" => "ellipse",
        "diamond" => "diamond",
        "isosceles-triangle" => "triangle",
        "right-triangle" => "rtTriangle",
        "parallelogram" => "parallelogram",
        "trapezoid" => "trapezoid",
        "hexagon" => "hexagon",
        "octagon" => "octagon",
        "pentagon" => "pentagon",
        "star5" => "star5",
        "right-arrow" => "rightArrow",
        "left-arrow" => "leftArrow",
        "up-arrow" => "upArrow",
        "down-arrow" => "downArrow",
        "left-right-arrow" => "leftRightArrow",
        "heart" => "heart",
        "cloud" => "cloud",
        _ => "rect",
    }
}

fn map_preset_to_odp_geom(preset: &str) -> &'static str {
    match preset {
        "roundRect" => "round-rectangle",
        "ellipse" => "ellipse",
        "diamond" => "diamond",
        "triangle" => "isosceles-triangle",
        "rtTriangle" => "right-triangle",
        "parallelogram" => "parallelogram",
        "trapezoid" => "trapezoid",
        "hexagon" => "hexagon",
        "octagon" => "octagon",
        "pentagon" => "pentagon",
        "star5" => "star5",
        "rightArrow" => "right-arrow",
        "leftArrow" => "left-arrow",
        "upArrow" => "up-arrow",
        "downArrow" => "down-arrow",
        "leftRightArrow" => "left-right-arrow",
        "heart" => "heart",
        "cloud" => "cloud",
        _ => "rectangle",
    }
}

fn apply_graphic_style(sh: &mut Shape, g: &OdpGraphicStyle) {
    if g.fill_none {
        sh.fill = Some(Fill::None);
    } else if let Some(mut c) = g.fill_color {
        if let Some(op) = g.fill_opacity {
            c.a = ((f64::from(c.a) * op).round().clamp(0.0, 255.0)) as u8;
        }
        let mut cr = ColorRef::rgb(Rgba::rgb(c.r, c.g, c.b));
        if c.a < 255 {
            cr = cr.with(deckcraft_model::ColorTransform::Alpha(((f64::from(c.a) / 255.0) * 100_000.0).round() as i32));
        }
        sh.fill = Some(Fill::solid(cr));
    }
    if g.stroke_none {
        sh.line = Some(Line::none());
    } else if g.stroke_color.is_some() || g.stroke_width.is_some() || g.stroke_dashed {
        let col = g.stroke_color.unwrap_or(Rgba::BLACK);
        sh.line = Some(Line {
            fill: Some(Fill::solid(ColorRef::rgb(col))),
            width: Some(g.stroke_width.unwrap_or(1.0)),
            dash: g.stroke_dashed.then(|| Dash::Preset { name: "dash".into() }),
            ..Default::default()
        });
    }
}

fn content_type_for_name(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "application/octet-stream"
    }
}

fn ext_for_content_type(ct: &str, fallback_name: &str) -> &'static str {
    if ct.contains("png") {
        "png"
    } else if ct.contains("jpeg") || ct.contains("jpg") {
        "jpg"
    } else if ct.contains("gif") {
        "gif"
    } else if ct.contains("webp") {
        "webp"
    } else if ct.contains("svg") {
        "svg"
    } else if fallback_name.to_ascii_lowercase().ends_with(".jpg") || fallback_name.to_ascii_lowercase().ends_with(".jpeg") {
        "jpg"
    } else {
        "png"
    }
}

#[derive(Default)]
struct ParaBuild {
    para: Paragraph,
    span_stack: Vec<RunProps>,
    link_stack: Vec<String>,
}

impl ParaBuild {
    fn current_props(&self, base: Option<&RunProps>) -> RunProps {
        let mut p = base.cloned().unwrap_or_default();
        for s in &self.span_stack {
            let mut next = s.clone();
            next.inherit(&p);
            p = next;
        }
        if let Some(url) = self.link_stack.last()
            && deckcraft_model::text::safe_url(url)
        {
            p.link = Some(Hyperlink {
                action: Action::Url { url: url.clone() },
                tooltip: String::new(),
                highlight_click: false,
                sound: None,
                stop_sound: false,
                zoom_return: None,
            });
        }
        p
    }

    fn push_text(&mut self, text: &str, base: Option<&RunProps>) {
        if text.is_empty() {
            return;
        }
        let props = self.current_props(base);
        if let Some(last) = self.para.runs.last_mut()
            && matches!(last.kind, RunKind::Text)
            && last.props == props
        {
            last.text.push_str(text);
        } else {
            self.para.runs.push(Run { text: text.to_string(), props, kind: RunKind::Text });
        }
    }
}

/// Read an OpenDocument Presentation (`.odp`) byte buffer into a [`Presentation`].
pub fn load_odp(bytes: &[u8]) -> Result<Presentation> {
    if !sniff_odp(bytes) {
        return Err(FormatError::NotOurs("not an OpenDocument presentation (.odp)".into()));
    }
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| FormatError::Damaged(e.to_string()))?;
    let content_xml =
        read_zip_entry(&mut z, "content.xml", MAX_ENTRY)?.ok_or_else(|| FormatError::Damaged("missing content.xml in .odp archive".into()))?;
    let styles_xml = read_zip_entry(&mut z, "styles.xml", MAX_ENTRY)?;
    let meta_xml = read_zip_entry(&mut z, "meta.xml", MAX_ENTRY)?;

    let mut cat = OdpStyleCatalog::default();
    if let Some(sx) = &styles_xml {
        parse_styles_into(sx, &mut cat);
    }
    parse_styles_into(&content_xml, &mut cat);

    let size = cat.page_size.unwrap_or(defaults::WIDE);
    let mut pres = defaults::blank_presentation(size, Default::default(), false);
    if let Some(mx) = &meta_xml {
        parse_meta_xml(mx, &mut pres);
    }
    if let Some(bg) = cat.master_bg
        && let Some(m) = pres.masters.first_mut()
    {
        Arc::make_mut(m).background = Some(Background::Fill { fill: Fill::solid(ColorRef::rgb(bg)) });
    }

    // Collect media paths referenced in archive (`Pictures/*` or `media/*`).
    let mut archive_media: HashMap<String, MediaId> = HashMap::new();
    let mut media_bytes_used = 0u64;
    let names: Vec<String> = (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string())).collect();
    for entry_name in names {
        let is_media = entry_name.starts_with("Pictures/") || entry_name.starts_with("media/");
        if !is_media || entry_name.ends_with('/') {
            continue;
        }
        if pres.media.len() >= MAX_MEDIA_ITEMS {
            return Err(FormatError::Damaged("too many media items in .odp".into()));
        }
        if let Some(data) = read_zip_entry(&mut z, &entry_name, MAX_ENTRY)? {
            media_bytes_used = media_bytes_used.saturating_add(data.len() as u64);
            if media_bytes_used > MAX_TOTAL_MEDIA {
                return Err(FormatError::Damaged("media in .odp exceeds total size limit".into()));
            }
            let file_only = entry_name.rsplit('/').next().unwrap_or(&entry_name);
            let ct = content_type_for_name(file_only);
            if let Some(id) = pres.add_media(file_only, ct, data) {
                archive_media.insert(entry_name, id);
            }
        }
    }

    let default_layout = pres
        .masters
        .first()
        .and_then(|m| m.layouts.iter().find(|l| l.kind == deckcraft_model::LayoutType::Blank).or_else(|| m.layouts.first()).map(|l| l.id))
        .unwrap_or(LayoutId(1));

    parse_content_pages(&content_xml, &cat, &archive_media, default_layout, &mut pres);
    if pres.slides.is_empty() {
        let s = defaults::new_slide(&mut pres, default_layout).unwrap_or_default();
        pres.slides.push(Arc::new(s));
    }
    repair(&mut pres);
    Ok(pres)
}

fn parse_content_pages(
    xml: &[u8],
    cat: &OdpStyleCatalog,
    archive_media: &HashMap<String, MediaId>,
    default_layout: LayoutId,
    pres: &mut Presentation,
) {
    let mut r = Reader::from_reader(xml);
    r.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut cur_slide: Option<Slide> = None;
    let mut in_notes = false;
    let mut notes_paras: Vec<Paragraph> = Vec::new();

    let mut cur_shape: Option<Shape> = None;
    let mut cur_shape_gstyle: Option<OdpGraphicStyle> = None;
    let mut shape_paras: Vec<Paragraph> = Vec::new();
    let mut shape_link: Option<String> = None;

    let mut cur_table: Option<Table> = None;
    let mut cur_row: Option<Row> = None;
    let mut cur_cell: Option<Cell> = None;
    let mut cell_paras: Vec<Paragraph> = Vec::new();

    let mut cur_para: Option<ParaBuild> = None;
    let mut cur_para_base_run: Option<RunProps> = None;
    let mut list_stack: Vec<String> = Vec::new();
    let mut ph_idx_counter = 0u32;

    let finish_shape = |mut sh: Shape, gstyle: Option<OdpGraphicStyle>, paras: Vec<Paragraph>, tbl: Option<Table>, slide: &mut Slide| {
        if let Some(table) = tbl {
            sh.kind = ShapeKind::Table(table);
        } else if !matches!(sh.kind, ShapeKind::Picture { .. }) && (!paras.is_empty() || sh.text_box || sh.ph.is_some()) {
            let mut body = TextBody { paragraphs: paras, ..Default::default() };
            if let Some(gs) = &gstyle
                && let Some(anc) = gs.anchor
            {
                body.body.anchor = Some(anc);
            }
            sh.text = Some(body);
        }
        slide.shapes.push(sh);
    };

    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let ln = local_name(qname.as_ref());
                match ln {
                    b"page" => {
                        let sid = SlideId(pres.alloc_id().unwrap_or(1));
                        let name = attr_str(&e, b"name").unwrap_or_default();
                        let mut sl = Slide { id: sid, layout: default_layout, name, show_master_shapes: true, ..Default::default() };
                        if let Some(sn) = attr_str(&e, b"style-name")
                            && let Some(ps) = cat.pages.get(&sn)
                        {
                            if let Some(bg) = ps.bg_color {
                                sl.background = Some(Background::Fill { fill: Fill::solid(ColorRef::rgb(bg)) });
                            }
                            sl.hidden = ps.hidden;
                            sl.transition.clone_from(&ps.transition);
                        }
                        ph_idx_counter = 0;
                        cur_slide = Some(sl);
                        in_notes = false;
                        notes_paras.clear();
                    }
                    b"notes" if cur_slide.is_some() => {
                        in_notes = true;
                    }
                    b"a" if cur_slide.is_some() && cur_para.is_none() && cur_shape.is_none() => {
                        shape_link = attr_str(&e, b"href");
                    }
                    b"frame" | b"custom-shape" | b"rect" | b"ellipse" | b"circle" | b"line"
                        if cur_slide.is_some() && !in_notes && cur_shape.is_none() =>
                    {
                        let id = ShapeId(pres.alloc_id().unwrap_or(1));
                        let xfrm = parse_xfrm(&e);
                        let is_frame = ln == b"frame";
                        let preset = match ln {
                            b"ellipse" | b"circle" => "ellipse",
                            b"line" => "line",
                            _ => "rect",
                        };
                        let mut sh = Shape {
                            id,
                            name: attr_str(&e, b"name").unwrap_or_default(),
                            xfrm: Some(xfrm),
                            geom: Geom::preset(preset),
                            text_box: is_frame,
                            fill: is_frame.then_some(Fill::None),
                            line: is_frame.then(Line::none),
                            ..Default::default()
                        };
                        if ln == b"line" {
                            sh.kind = ShapeKind::Connector { start: None, end: None };
                        }
                        if let Some(cls) = attr_str(&e, b"class") {
                            let kind = match cls.as_str() {
                                "title" => Some(PhType::Title),
                                "subtitle" => Some(PhType::SubTitle),
                                "outline" => Some(PhType::Body),
                                _ => None,
                            };
                            if let Some(k) = kind {
                                ph_idx_counter = ph_idx_counter.saturating_add(1);
                                sh.ph = Some(Placeholder { kind: k, idx: ph_idx_counter, ..Default::default() });
                            }
                        }
                        let gstyle = attr_str(&e, b"style-name").and_then(|sn| cat.graphics.get(&sn).cloned());
                        if let Some(gs) = &gstyle {
                            apply_graphic_style(&mut sh, gs);
                        }
                        if let Some(url) = &shape_link
                            && deckcraft_model::text::safe_url(url)
                        {
                            sh.click = Some(Hyperlink {
                                action: Action::Url { url: url.clone() },
                                tooltip: String::new(),
                                highlight_click: false,
                                sound: None,
                                stop_sound: false,
                                zoom_return: None,
                            });
                        }
                        cur_shape = Some(sh);
                        cur_shape_gstyle = gstyle;
                        shape_paras.clear();
                    }
                    b"enhanced-geometry" => {
                        if let Some(sh) = cur_shape.as_mut()
                            && let Some(t) = attr_str(&e, b"type")
                        {
                            sh.geom = Geom::preset(map_odp_geom_to_preset(&t));
                        }
                    }
                    b"image" if !in_notes => {
                        if let Some(sh) = cur_shape.as_mut()
                            && let Some(href) = attr_str(&e, b"href")
                        {
                            let clean = href.trim_start_matches("./");
                            if let Some(&mid) = archive_media.get(clean) {
                                sh.kind = ShapeKind::Picture { fill: PictureFill { media: mid, ..Default::default() } };
                                sh.text_box = false;
                            }
                        }
                    }
                    b"table" if !in_notes && cur_shape.is_some() => {
                        cur_table = Some(Table { cols: Vec::new(), rows: Vec::new(), ..Default::default() });
                    }
                    b"table-column" => {
                        if let Some(tbl) = cur_table.as_mut() {
                            let repeat = attr_str(&e, b"number-columns-repeated").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 64);
                            let col_w = attr_str(&e, b"style-name").and_then(|_| None::<f64>).unwrap_or(120.0);
                            for _ in 0..repeat {
                                tbl.cols.push(col_w);
                            }
                        }
                    }
                    b"table-row" => {
                        if cur_table.is_some() {
                            cur_row = Some(Row { height: 28.0, cells: Vec::new() });
                        }
                    }
                    b"table-cell" => {
                        if cur_row.is_some() {
                            let mut cell = Cell { grid_span: 1, row_span: 1, ..Default::default() };
                            if let Some(sn) = attr_str(&e, b"style-name")
                                && let Some(gs) = cat.graphics.get(&sn)
                                && let Some(c) = gs.fill_color
                            {
                                cell.fill = Some(Fill::solid(ColorRef::rgb(c)));
                            }
                            cur_cell = Some(cell);
                            cell_paras.clear();
                        }
                    }
                    b"covered-table-cell" => {
                        if let Some(row) = cur_row.as_mut() {
                            row.cells.push(Cell { h_merge: true, grid_span: 1, row_span: 1, ..Default::default() });
                        }
                    }
                    b"list" => {
                        let lname = attr_str(&e, b"style-name").or_else(|| list_stack.last().cloned()).unwrap_or_default();
                        list_stack.push(lname);
                    }
                    b"p" | b"h" if in_notes || cur_shape.is_some() => {
                        let mut pb = ParaBuild::default();
                        let style_name = attr_str(&e, b"style-name");
                        if let Some(sn) = &style_name {
                            if let Some(pp) = cat.paras.get(sn) {
                                pb.para.props = pp.clone();
                            }
                            cur_para_base_run = cat.texts.get(sn).cloned();
                        } else {
                            cur_para_base_run = None;
                        }
                        if !list_stack.is_empty() && !in_notes {
                            let lvl = (list_stack.len() - 1).min(8);
                            pb.para.level = lvl as u8;
                            if let Some(lname) = list_stack.first()
                                && let Some(bullets) = cat.list_bullets.get(lname)
                                && let Some(b) = bullets.get(lvl).or_else(|| bullets.last())
                            {
                                pb.para.props.bullet = Some(b.clone());
                            } else {
                                pb.para.props.bullet = Some(Bullet::Char { char: "•".into() });
                            }
                        }
                        cur_para = Some(pb);
                    }
                    b"span" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let rp = attr_str(&e, b"style-name").and_then(|sn| cat.texts.get(&sn).cloned()).unwrap_or_default();
                            pb.span_stack.push(rp);
                        }
                    }
                    b"a" if cur_para.is_some() => {
                        if let Some(pb) = cur_para.as_mut() {
                            pb.link_stack.push(attr_str(&e, b"href").unwrap_or_default());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let ln = local_name(qname.as_ref());
                match ln {
                    b"enhanced-geometry" => {
                        if let Some(sh) = cur_shape.as_mut()
                            && let Some(t) = attr_str(&e, b"type")
                        {
                            sh.geom = Geom::preset(map_odp_geom_to_preset(&t));
                        }
                    }
                    b"image" if !in_notes => {
                        if let Some(sh) = cur_shape.as_mut()
                            && let Some(href) = attr_str(&e, b"href")
                        {
                            let clean = href.trim_start_matches("./");
                            if let Some(&mid) = archive_media.get(clean) {
                                sh.kind = ShapeKind::Picture { fill: PictureFill { media: mid, ..Default::default() } };
                                sh.text_box = false;
                            }
                        }
                    }
                    b"table-column" => {
                        if let Some(tbl) = cur_table.as_mut() {
                            let repeat = attr_str(&e, b"number-columns-repeated").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 64);
                            for _ in 0..repeat {
                                tbl.cols.push(120.0);
                            }
                        }
                    }
                    b"covered-table-cell" => {
                        if let Some(row) = cur_row.as_mut() {
                            row.cells.push(Cell { h_merge: true, grid_span: 1, row_span: 1, ..Default::default() });
                        }
                    }
                    b"line-break" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let props = pb.current_props(cur_para_base_run.as_ref());
                            pb.para.runs.push(Run { text: "\n".into(), props, kind: RunKind::Break });
                        }
                    }
                    b"tab" => {
                        if let Some(pb) = cur_para.as_mut() {
                            pb.push_text("\t", cur_para_base_run.as_ref());
                        }
                    }
                    b"s" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let count = attr_str(&e, b"c").and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 256);
                            pb.push_text(&" ".repeat(count), cur_para_base_run.as_ref());
                        }
                    }
                    b"page-number" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let props = pb.current_props(cur_para_base_run.as_ref());
                            pb.para.runs.push(Run { text: "1".into(), props, kind: RunKind::Field { field: "slidenum".into() } });
                        }
                    }
                    b"p" | b"h" if in_notes || cur_shape.is_some() => {
                        let para = Paragraph::default();
                        if in_notes {
                            notes_paras.push(para);
                        } else if cur_cell.is_some() {
                            cell_paras.push(para);
                        } else if cur_shape.is_some() {
                            shape_paras.push(para);
                        }
                    }
                    b"rect" | b"ellipse" | b"circle" | b"line" | b"custom-shape" if cur_slide.is_some() && !in_notes && cur_shape.is_none() => {
                        let id = ShapeId(pres.alloc_id().unwrap_or(1));
                        let xfrm = parse_xfrm(&e);
                        let preset = match ln {
                            b"ellipse" | b"circle" => "ellipse",
                            b"line" => "line",
                            _ => "rect",
                        };
                        let mut sh = Shape { id, xfrm: Some(xfrm), geom: Geom::preset(preset), ..Default::default() };
                        if ln == b"line" {
                            sh.kind = ShapeKind::Connector { start: None, end: None };
                        }
                        if let Some(sn) = attr_str(&e, b"style-name")
                            && let Some(gs) = cat.graphics.get(&sn)
                        {
                            apply_graphic_style(&mut sh, gs);
                        }
                        if let Some(sl) = cur_slide.as_mut() {
                            sl.shapes.push(sh);
                        }
                    }
                    b"page" => {
                        let sid = SlideId(pres.alloc_id().unwrap_or(1));
                        let name = attr_str(&e, b"name").unwrap_or_default();
                        let mut sl = Slide { id: sid, layout: default_layout, name, show_master_shapes: true, ..Default::default() };
                        if let Some(sn) = attr_str(&e, b"style-name")
                            && let Some(ps) = cat.pages.get(&sn)
                        {
                            if let Some(bg) = ps.bg_color {
                                sl.background = Some(Background::Fill { fill: Fill::solid(ColorRef::rgb(bg)) });
                            }
                            sl.hidden = ps.hidden;
                            sl.transition.clone_from(&ps.transition);
                        }
                        pres.slides.push(Arc::new(sl));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(pb) = cur_para.as_mut()
                    && let Ok(raw) = t.decode()
                {
                    let s = quick_xml::escape::unescape(&raw).map(|c| c.into_owned()).unwrap_or_else(|_| raw.into_owned());
                    pb.push_text(&s, cur_para_base_run.as_ref());
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let ln = local_name(qname.as_ref());
                match ln {
                    b"span" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let _ = pb.span_stack.pop();
                        }
                    }
                    b"a" => {
                        if let Some(pb) = cur_para.as_mut() {
                            let _ = pb.link_stack.pop();
                        } else {
                            shape_link = None;
                        }
                    }
                    b"p" | b"h" => {
                        if let Some(pb) = cur_para.take() {
                            cur_para_base_run = None;
                            if in_notes {
                                notes_paras.push(pb.para);
                            } else if cur_cell.is_some() {
                                cell_paras.push(pb.para);
                            } else if cur_shape.is_some() {
                                shape_paras.push(pb.para);
                            }
                        }
                    }
                    b"list" => {
                        let _ = list_stack.pop();
                    }
                    b"table-cell" => {
                        if let (Some(mut cell), Some(row)) = (cur_cell.take(), cur_row.as_mut()) {
                            cell.text = TextBody { paragraphs: std::mem::take(&mut cell_paras), ..Default::default() };
                            row.cells.push(cell);
                        }
                    }
                    b"table-row" => {
                        if let (Some(row), Some(tbl)) = (cur_row.take(), cur_table.as_mut()) {
                            tbl.rows.push(row);
                        }
                    }
                    b"table" => {
                        if let (Some(tbl), Some(sh)) = (cur_table.as_mut(), cur_shape.as_ref()) {
                            let n_cols = tbl.rows.iter().map(|rw| rw.cells.len()).max().unwrap_or(0);
                            if tbl.cols.len() < n_cols {
                                tbl.cols.resize(n_cols, 120.0);
                            }
                            if let Some(xf) = sh.xfrm
                                && n_cols > 0
                                && xf.w > 1.0
                            {
                                let sum: f64 = tbl.cols.iter().sum();
                                if sum > 0.0 {
                                    let scale = xf.w / sum;
                                    for c in &mut tbl.cols {
                                        *c *= scale;
                                    }
                                }
                                if !tbl.rows.is_empty() && xf.h > 1.0 {
                                    let rh = xf.h / tbl.rows.len() as f64;
                                    for rw in &mut tbl.rows {
                                        rw.height = rh;
                                    }
                                }
                            }
                        }
                    }
                    b"frame" | b"custom-shape" | b"rect" | b"ellipse" | b"circle" | b"line" if !in_notes => {
                        if let (Some(sh), Some(sl)) = (cur_shape.take(), cur_slide.as_mut()) {
                            finish_shape(sh, cur_shape_gstyle.take(), std::mem::take(&mut shape_paras), cur_table.take(), sl);
                        }
                    }
                    b"notes" => {
                        in_notes = false;
                        if let Some(sl) = cur_slide.as_mut() {
                            sl.notes = TextBody { paragraphs: std::mem::take(&mut notes_paras), ..Default::default() };
                        }
                    }
                    b"page" => {
                        if let Some(sl) = cur_slide.take() {
                            pres.slides.push(Arc::new(sl));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

// -----------------------------------------------------------------------------
// ODP Writer (`save_odp`)
// -----------------------------------------------------------------------------

fn xml_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\r' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

fn hex_rgb(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn resolve_solid_fill(ctx: &resolve::Ctx<'_>, fill: &Fill, ph: Option<Rgba>) -> Option<(Rgba, f64)> {
    match fill {
        Fill::None => None,
        Fill::Solid { color } => {
            let rgba = ctx.color(color, ph);
            let op = f64::from(rgba.a) / 255.0;
            Some((rgba, op))
        }
        Fill::Gradient(g) => g.stops.first().map(|st| {
            let rgba = ctx.color(&st.color, ph);
            (rgba, f64::from(rgba.a) / 255.0)
        }),
        Fill::Pattern(p) => {
            let rgba = ctx.color(&p.fg, ph);
            Some((rgba, f64::from(rgba.a) / 255.0))
        }
        _ => None,
    }
}

fn odp_transition_attrs(tr: &Transition) -> String {
    let mut attrs = String::new();
    let dur_s = (f64::from(tr.duration_ms.max(100)) / 1000.0).clamp(0.1, 60.0);
    let (smil_type, smil_sub, rev): (&str, &str, bool) = match tr.kind.as_str() {
        "none" => ("none", "", false),
        "cut" => ("cut", "", false),
        "fade" => ("fade", if tr.option == "throughBlack" { "fadeOverColor" } else { "crossfade" }, false),
        "push" => match tr.option.as_str() {
            "r" => ("pushWipe", "fromLeft", false),
            "l" => ("pushWipe", "fromRight", false),
            "d" => ("pushWipe", "fromTop", false),
            _ => ("pushWipe", "fromBottom", false),
        },
        "wipe" => match tr.option.as_str() {
            "l" => ("barWipe", "leftToRight", true),
            "d" => ("barWipe", "topToBottom", false),
            "u" => ("barWipe", "topToBottom", true),
            _ => ("barWipe", "leftToRight", false),
        },
        "split" => match tr.option.as_str() {
            "vertIn" => ("barnDoorWipe", "vertical", true),
            "horzOut" => ("barnDoorWipe", "horizontal", false),
            "horzIn" => ("barnDoorWipe", "horizontal", true),
            _ => ("barnDoorWipe", "vertical", false),
        },
        "uncover" => match tr.option.as_str() {
            "r" => ("slideWipe", "fromLeft", true),
            "l" => ("slideWipe", "fromRight", true),
            "d" => ("slideWipe", "fromTop", true),
            _ => ("slideWipe", "fromBottom", true),
        },
        "cover" => match tr.option.as_str() {
            "r" => ("slideWipe", "fromLeft", false),
            "l" => ("slideWipe", "fromRight", false),
            "d" => ("slideWipe", "fromTop", false),
            _ => ("slideWipe", "fromBottom", false),
        },
        "zoom" => ("irisWipe", "rectangle", tr.option == "out"),
        "dissolve" => ("dissolve", "", false),
        "checker" => ("checkerBoardWipe", if tr.option == "vert" { "down" } else { "across" }, false),
        "blinds" => ("blindsWipe", if tr.option == "horz" { "horizontal" } else { "vertical" }, false),
        "clock" => ("clockWipe", "clockwiseTwelve", false),
        "morph" => ("morph", "", false),
        _ => ("fade", "crossfade", false),
    };
    if smil_type != "none" {
        attrs.push_str(&format!(" smil:type=\"{smil_type}\""));
        if !smil_sub.is_empty() {
            attrs.push_str(&format!(" smil:subtype=\"{smil_sub}\""));
        }
        if rev {
            attrs.push_str(" smil:direction=\"reverse\"");
        }
        attrs.push_str(&format!(" smil:dur=\"{dur_s:.3}s\""));
    }
    if let Some(adv) = tr.advance_after_ms {
        let secs = f64::from(adv) / 1000.0;
        let kind = if tr.advance_on_click { "semi-automatic" } else { "automatic" };
        attrs.push_str(&format!(" presentation:transition-type=\"{kind}\" presentation:duration=\"PT{secs:.3}S\""));
    } else if !tr.advance_on_click {
        attrs.push_str(" presentation:transition-type=\"manual\"");
    }
    attrs
}

fn write_xfrm_attrs(xf: &Xfrm) -> String {
    let w = xf.w.max(0.0);
    let h = xf.h.max(0.0);
    let rot = xf.rot.rem_euclid(360.0);
    if rot.abs() < 1e-6 || (rot - 360.0).abs() < 1e-6 {
        format!("svg:x=\"{:.3}pt\" svg:y=\"{:.3}pt\" svg:width=\"{w:.3}pt\" svg:height=\"{h:.3}pt\"", xf.x, xf.y)
    } else {
        let theta = rot.to_radians();
        let cx = xf.x + w * 0.5;
        let cy = xf.y + h * 0.5;
        let tx = cx - (w * 0.5) * theta.cos() + (h * 0.5) * theta.sin();
        let ty = cy - (w * 0.5) * theta.sin() - (h * 0.5) * theta.cos();
        let alpha = -theta;
        format!("svg:width=\"{w:.3}pt\" svg:height=\"{h:.3}pt\" draw:transform=\"rotate ({alpha:.7}) translate ({tx:.3}pt {ty:.3}pt)\"")
    }
}

struct OdpStyleBuilder {
    xml: String,
    next_p: usize,
    next_t: usize,
    next_g: usize,
    next_dp: usize,
    next_l: usize,
}

impl OdpStyleBuilder {
    fn new() -> Self {
        OdpStyleBuilder { xml: String::new(), next_p: 1, next_t: 1, next_g: 1, next_dp: 1, next_l: 1 }
    }

    fn add_page_style(&mut self, ctx: Option<&resolve::Ctx<'_>>, sl: &Slide) -> String {
        let id = format!("dp{}", self.next_dp);
        self.next_dp += 1;
        let mut props = String::new();
        if let Some(ctx) = ctx {
            let (bg, ph) = resolve::background(ctx, Some(sl));
            if let Some((c, _)) = resolve_solid_fill(ctx, &bg, ph) {
                props.push_str(&format!(" draw:fill=\"solid\" draw:fill-color=\"{}\"", hex_rgb(c)));
            }
        }
        if sl.hidden {
            props.push_str(" presentation:visibility=\"hidden\"");
        }
        if let Some(tr) = &sl.transition {
            props.push_str(&odp_transition_attrs(tr));
        }
        self.xml.push_str(&format!(
            "<style:style style:name=\"{id}\" style:family=\"drawing-page\"><style:drawing-page-properties{props}/></style:style>"
        ));
        id
    }

    fn add_graphic_style(&mut self, ctx: &resolve::Ctx<'_>, sh: &Shape) -> String {
        let id = format!("gr{}", self.next_g);
        self.next_g += 1;
        let mut props = String::new();
        let (fill, fph) = resolve::fill(ctx, sh);
        if fill.as_ref().is_none_or(|f| matches!(f, Fill::None)) || matches!(sh.kind, ShapeKind::Picture { .. }) {
            props.push_str(" draw:fill=\"none\"");
        } else if let Some(f) = &fill
            && let Some((c, op)) = resolve_solid_fill(ctx, f, fph)
        {
            props.push_str(&format!(" draw:fill=\"solid\" draw:fill-color=\"{}\"", hex_rgb(c)));
            if op < 0.999 {
                props.push_str(&format!(" draw:opacity=\"{:.1}%\"", (op * 100.0).clamp(0.0, 100.0)));
            }
        } else {
            props.push_str(" draw:fill=\"none\"");
        }
        let (ln, lph) = resolve::line(ctx, sh);
        if let Some(lf) = &ln.fill
            && !matches!(lf, Fill::None)
            && let Some((c, _)) = resolve_solid_fill(ctx, lf, lph)
        {
            let stroke_kind = if ln.dash.as_ref().is_some_and(|d| !d.pattern().is_empty()) { "dash" } else { "solid" };
            let w = ln.width.unwrap_or(1.0);
            props.push_str(&format!(" draw:stroke=\"{stroke_kind}\" draw:stroke-color=\"{}\" svg:stroke-width=\"{w:.2}pt\"", hex_rgb(c)));
        } else {
            props.push_str(" draw:stroke=\"none\"");
        }
        if sh.text.is_some() {
            let bp = resolve::body(ctx, sh);
            if let Some(anc) = bp.anchor {
                let va = match anc {
                    Anchor::Top => "top",
                    Anchor::Middle => "middle",
                    Anchor::Bottom => "bottom",
                    Anchor::Justified | Anchor::Distributed => "justify",
                };
                props.push_str(&format!(" draw:textarea-vertical-align=\"{va}\""));
            }
        }
        self.xml.push_str(&format!("<style:style style:name=\"{id}\" style:family=\"graphic\"><style:graphic-properties{props}/></style:style>"));
        id
    }

    fn add_para_style(&mut self, pp: &ParaProps) -> String {
        let id = format!("P{}", self.next_p);
        self.next_p += 1;
        let mut props = String::new();
        if let Some(al) = pp.align {
            let v = match al {
                Align::Left => "start",
                Align::Center => "center",
                Align::Right => "end",
                Align::Justify | Align::JustLow | Align::Distributed | Align::ThaiDistributed => "justify",
            };
            props.push_str(&format!(" fo:text-align=\"{v}\""));
        }
        if let Some(ml) = pp.margin_left
            && ml > 0.0
        {
            props.push_str(&format!(" fo:margin-left=\"{ml:.2}pt\""));
        }
        self.xml.push_str(&format!("<style:style style:name=\"{id}\" style:family=\"paragraph\"><style:paragraph-properties{props}/></style:style>"));
        id
    }

    fn add_run_style(&mut self, ctx: &resolve::Ctx<'_>, rp: &RunProps) -> String {
        let id = format!("T{}", self.next_t);
        self.next_t += 1;
        let mut props = String::new();
        let fam = resolve::font_family(ctx, rp);
        if !fam.is_empty() {
            props.push_str(&format!(" style:font-name=\"{}\" fo:font-family=\"{}\"", xml_esc(&fam), xml_esc(&fam)));
        }
        if let Some(sz) = rp.size {
            props.push_str(&format!(" fo:font-size=\"{sz:.2}pt\""));
        }
        if rp.bold == Some(true) {
            props.push_str(" fo:font-weight=\"bold\"");
        }
        if rp.italic == Some(true) {
            props.push_str(" fo:font-style=\"italic\"");
        }
        if rp.underline.as_deref().is_some_and(|u| u != "none") {
            props.push_str(" style:text-underline-style=\"solid\"");
        }
        if rp.strike.is_some_and(|s| s != Strike::None) {
            props.push_str(" style:text-line-through-style=\"solid\"");
        }
        let tc = resolve::text_color(ctx, rp);
        if tc.a > 0 {
            props.push_str(&format!(" fo:color=\"{}\"", hex_rgb(tc)));
        }
        self.xml.push_str(&format!("<style:style style:name=\"{id}\" style:family=\"text\"><style:text-properties{props}/></style:style>"));
        id
    }

    fn add_list_style(&mut self, bullet: &Bullet) -> String {
        let id = format!("L{}", self.next_l);
        self.next_l += 1;
        self.xml.push_str(&format!("<text:list-style style:name=\"{id}\">"));
        for lvl in 1..=9 {
            match bullet {
                Bullet::AutoNum { .. } => {
                    self.xml.push_str(&format!("<text:list-level-style-number text:level=\"{lvl}\" style:num-suffix=\".\" style:num-format=\"1\"/>"));
                }
                Bullet::Char { char: ch } => {
                    self.xml.push_str(&format!("<text:list-level-style-bullet text:level=\"{lvl}\" text:bullet-char=\"{}\"/>", xml_esc(ch)));
                }
                _ => {
                    self.xml.push_str(&format!("<text:list-level-style-bullet text:level=\"{lvl}\" text:bullet-char=\"•\"/>"));
                }
            }
        }
        self.xml.push_str("</text:list-style>");
        id
    }
}

fn write_paragraphs_xml(ctx: &resolve::Ctx<'_>, sh: &Shape, body: &TextBody, sb: &mut OdpStyleBuilder, out: &mut String) {
    for p in &body.paragraphs {
        let resolved_pp = resolve::para(ctx, sh, p);
        let p_style = sb.add_para_style(&resolved_pp);
        let bullet = resolved_pp.bullet.as_ref().filter(|b| !matches!(b, Bullet::None));
        let depth = if bullet.is_some() { usize::from(p.level) + 1 } else { 0 };
        let l_style = bullet.map(|b| sb.add_list_style(b));
        for d in 0..depth {
            if d == 0
                && let Some(ls) = &l_style
            {
                out.push_str(&format!("<text:list text:style-name=\"{ls}\"><text:list-item>"));
            } else {
                out.push_str("<text:list><text:list-item>");
            }
        }
        out.push_str(&format!("<text:p text:style-name=\"{p_style}\">"));
        for run in &p.runs {
            match &run.kind {
                RunKind::Break => out.push_str("<text:line-break/>"),
                RunKind::Field { field } if field == "slidenum" => out.push_str("<text:page-number/>"),
                _ => {
                    let rp = resolve::run(ctx, sh, p, &run.props);
                    let t_style = sb.add_run_style(ctx, &rp);
                    let link_url = rp.link.as_ref().and_then(|h| match &h.action {
                        Action::Url { url } if deckcraft_model::text::safe_url(url) => Some(url.as_str()),
                        _ => None,
                    });
                    if let Some(u) = link_url {
                        out.push_str(&format!("<text:a xlink:type=\"simple\" xlink:href=\"{}\">", xml_esc(u)));
                    }
                    out.push_str(&format!("<text:span text:style-name=\"{t_style}\">"));
                    write_odf_text(&run.content(), out);
                    out.push_str("</text:span>");
                    if link_url.is_some() {
                        out.push_str("</text:a>");
                    }
                }
            }
        }
        out.push_str("</text:p>");
        for _ in 0..depth {
            out.push_str("</text:list-item></text:list>");
        }
    }
}

fn write_odf_text(text: &str, out: &mut String) {
    let mut space_run = 0usize;
    for ch in text.chars() {
        if ch == ' ' {
            space_run += 1;
            if space_run == 1 {
                out.push(' ');
            }
        } else {
            if space_run > 1 {
                out.push_str(&format!("<text:s text:c=\"{}\"/>", space_run - 1));
            }
            space_run = 0;
            match ch {
                '\t' => out.push_str("<text:tab/>"),
                '\n' => out.push_str("<text:line-break/>"),
                _ => out.push_str(&xml_esc(&ch.to_string())),
            }
        }
    }
    if space_run > 1 {
        out.push_str(&format!("<text:s text:c=\"{}\"/>", space_run - 1));
    }
}

fn write_shape_xml(ctx: &resolve::Ctx<'_>, sh: &Shape, media_paths: &HashMap<MediaId, String>, sb: &mut OdpStyleBuilder, out: &mut String) {
    if sh.hidden {
        return;
    }
    if let ShapeKind::Group { children, child } = &sh.kind {
        let gxf = resolve::xfrm(ctx, sh);
        let sx = if child.w.abs() > 1e-6 { gxf.w / child.w } else { 1.0 };
        let sy = if child.h.abs() > 1e-6 { gxf.h / child.h } else { 1.0 };
        out.push_str("<draw:g>");
        for c in children {
            let mut mapped = c.clone();
            if let Some(cxf) = c.xfrm {
                mapped.xfrm = Some(Xfrm {
                    x: gxf.x + (cxf.x - child.x) * sx,
                    y: gxf.y + (cxf.y - child.y) * sy,
                    w: cxf.w * sx,
                    h: cxf.h * sy,
                    rot: gxf.rot + cxf.rot,
                    ..cxf
                });
            }
            write_shape_xml(ctx, &mapped, media_paths, sb, out);
        }
        out.push_str("</draw:g>");
        return;
    }
    let xf = resolve::xfrm(ctx, sh);
    if sh.ph_type().is_some_and(PhType::is_footer_kind) {
        return;
    }
    let g_style = sb.add_graphic_style(ctx, sh);
    let xf_attrs = write_xfrm_attrs(&xf);
    let name_attr = if sh.name.is_empty() { String::new() } else { format!(" draw:name=\"{}\"", xml_esc(&sh.name)) };
    let ph_attr = match sh.ph_type() {
        Some(PhType::Title | PhType::CtrTitle) => " presentation:class=\"title\"",
        Some(PhType::SubTitle) => " presentation:class=\"subtitle\"",
        Some(PhType::Body | PhType::Obj) => " presentation:class=\"outline\"",
        _ => "",
    };
    let shape_url = sh.click.as_ref().and_then(|h| match &h.action {
        Action::Url { url } if deckcraft_model::text::safe_url(url) => Some(url.as_str()),
        _ => None,
    });
    if let Some(u) = shape_url {
        out.push_str(&format!("<draw:a xlink:type=\"simple\" xlink:href=\"{}\">", xml_esc(u)));
    }

    match &sh.kind {
        ShapeKind::Picture { fill } => {
            if let Some(p) = media_paths.get(&fill.media) {
                out.push_str(&format!(
                    "<draw:frame draw:style-name=\"{g_style}\"{name_attr} {xf_attrs}><draw:image xlink:href=\"{}\" xlink:type=\"simple\" xlink:show=\"embed\" xlink:actuate=\"onLoad\"/></draw:frame>",
                    xml_esc(p)
                ));
            }
        }
        ShapeKind::Table(tbl) => {
            out.push_str(&format!("<draw:frame draw:style-name=\"{g_style}\"{name_attr} {xf_attrs}><table:table>"));
            for _ in &tbl.cols {
                out.push_str("<table:table-column/>");
            }
            for row in &tbl.rows {
                out.push_str("<table:table-row>");
                for cell in &row.cells {
                    if cell.is_covered() {
                        out.push_str("<table:covered-table-cell/>");
                    } else {
                        out.push_str("<table:table-cell>");
                        let stand_in = Shape::table_cell(cell.text.clone());
                        write_paragraphs_xml(ctx, &stand_in, &cell.text, sb, out);
                        out.push_str("</table:table-cell>");
                    }
                }
                out.push_str("</table:table-row>");
            }
            out.push_str("</table:table></draw:frame>");
        }
        ShapeKind::Connector { .. } => {
            let (x1, y1, x2, y2) = (xf.x, xf.y, xf.x + xf.w, xf.y + xf.h);
            out.push_str(&format!(
                "<draw:line draw:style-name=\"{g_style}\"{name_attr} svg:x1=\"{x1:.3}pt\" svg:y1=\"{y1:.3}pt\" svg:x2=\"{x2:.3}pt\" svg:y2=\"{y2:.3}pt\"/>"
            ));
        }
        _ => {
            let preset = sh.geom.preset_name().unwrap_or("rect");
            let is_plain_frame = (sh.text_box || sh.ph.is_some()) && preset == "rect";
            if is_plain_frame {
                out.push_str(&format!("<draw:frame draw:style-name=\"{g_style}\"{name_attr}{ph_attr} {xf_attrs}><draw:text-box>"));
                if let Some(tb) = &sh.text {
                    write_paragraphs_xml(ctx, sh, tb, sb, out);
                }
                out.push_str("</draw:text-box></draw:frame>");
            } else {
                let odp_geom = map_preset_to_odp_geom(preset);
                out.push_str(&format!("<draw:custom-shape draw:style-name=\"{g_style}\"{name_attr} {xf_attrs}>"));
                if let Some(tb) = &sh.text {
                    write_paragraphs_xml(ctx, sh, tb, sb, out);
                }
                out.push_str(&format!("<draw:enhanced-geometry draw:type=\"{odp_geom}\"/>"));
                out.push_str("</draw:custom-shape>");
            }
        }
    }

    if shape_url.is_some() {
        out.push_str("</draw:a>");
    }
}

/// Serialize `pres` as a standards-compliant OASIS ODF 1.2 `.odp` presentation archive.
pub fn save_odp(pres: &Presentation) -> Result<Vec<u8>> {
    let mut media_paths: HashMap<MediaId, String> = HashMap::new();
    let mut manifest_entries: Vec<(String, String)> = Vec::new();
    for (idx, item) in pres.media.iter().enumerate() {
        if item.data.is_empty() || !item.content_type.starts_with("image/") && content_type_for_name(&item.name) == "application/octet-stream" {
            continue;
        }
        let ext = ext_for_content_type(&item.content_type, &item.name);
        let path = format!("Pictures/image{}.{ext}", idx + 1);
        let ct = if item.content_type.is_empty() { content_type_for_name(&path).to_string() } else { item.content_type.clone() };
        media_paths.insert(item.id, path.clone());
        manifest_entries.push((path, ct));
    }

    let (sw, sh) = (pres.slide_size.width.max(1.0), pres.slide_size.height.max(1.0));
    let orient = if sw >= sh { "landscape" } else { "portrait" };

    let mut sb = OdpStyleBuilder::new();
    let mut pages_xml = String::new();
    for (idx, sl) in pres.slides.iter().enumerate() {
        let ctx = resolve::Ctx::for_slide(pres, sl);
        let dp = sb.add_page_style(ctx.as_ref(), sl);
        let page_name = if sl.name.trim().is_empty() { format!("page{}", idx + 1) } else { sl.name.clone() };
        pages_xml.push_str(&format!("<draw:page draw:name=\"{}\" draw:style-name=\"{dp}\" draw:master-page-name=\"Default\">", xml_esc(&page_name)));
        if let Some(ctx) = &ctx {
            for shape in &sl.shapes {
                // Skip untouched empty placeholders.
                if shape.ph.is_some() && shape.text.as_ref().is_none_or(|t| t.text().trim().is_empty()) && matches!(shape.kind, ShapeKind::Shape) {
                    continue;
                }
                write_shape_xml(ctx, shape, &media_paths, &mut sb, &mut pages_xml);
            }
        }
        let notes_text = sl.notes_text();
        if !notes_text.trim().is_empty() {
            pages_xml.push_str("<presentation:notes><draw:frame presentation:class=\"notes\" svg:x=\"54pt\" svg:y=\"400pt\" svg:width=\"500pt\" svg:height=\"300pt\"><draw:text-box>");
            for line in notes_text.split('\n') {
                pages_xml.push_str("<text:p>");
                write_odf_text(line, &mut pages_xml);
                pages_xml.push_str("</text:p>");
            }
            pages_xml.push_str("</draw:text-box></draw:frame></presentation:notes>");
        }
        pages_xml.push_str("</draw:page>");
    }

    let ns = "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
              xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
              xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
              xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
              xmlns:draw=\"urn:oasis:names:tc:opendocument:xmlns:drawing:1.0\" \
              xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" \
              xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
              xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
              xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\" \
              xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\" \
              xmlns:presentation=\"urn:oasis:names:tc:opendocument:xmlns:presentation:1.0\" \
              xmlns:smil=\"urn:oasis:names:tc:opendocument:xmlns:smil-compatible:1.0\" \
              office:version=\"1.2\"";

    let content_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content {ns}>\
         <office:automatic-styles>{}</office:automatic-styles>\
         <office:body><office:presentation>{pages_xml}</office:presentation></office:body>\
         </office:document-content>",
        sb.xml
    );

    let styles_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-styles {ns}>\
         <office:automatic-styles>\
           <style:page-layout style:name=\"PM1\">\
             <style:page-layout-properties fo:page-width=\"{sw:.3}pt\" fo:page-height=\"{sh:.3}pt\" style:print-orientation=\"{orient}\"/>\
           </style:page-layout>\
         </office:automatic-styles>\
         <office:master-styles>\
           <style:master-page style:name=\"Default\" style:page-layout-name=\"PM1\"/>\
         </office:master-styles>\
         </office:document-styles>"
    );

    let meta_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-meta xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
           xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
           xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\" office:version=\"1.2\">\
           <office:meta>\
             <meta:generator>DeckCraft {}</meta:generator>\
             <dc:title>{}</dc:title>\
             <dc:subject>{}</dc:subject>\
             <dc:description>{}</dc:description>\
             <meta:initial-creator>{}</meta:initial-creator>\
             <dc:creator>{}</dc:creator>\
             {}\
           </office:meta>\
         </office:document-meta>",
        env!("CARGO_PKG_VERSION"),
        xml_esc(&pres.props.title),
        xml_esc(&pres.props.subject),
        xml_esc(&pres.props.comments),
        xml_esc(&pres.props.author),
        xml_esc(&pres.props.author),
        if pres.props.keywords.trim().is_empty() { String::new() } else { format!("<meta:keyword>{}</meta:keyword>", xml_esc(&pres.props.keywords)) }
    );

    let mut manifest_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.2\">\
         <manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.2\" manifest:media-type=\"application/vnd.oasis.opendocument.presentation\"/>\
         <manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>\
         <manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/>\
         <manifest:file-entry manifest:full-path=\"meta.xml\" manifest:media-type=\"text/xml\"/>",
    );
    for (p, ct) in &manifest_entries {
        manifest_xml.push_str(&format!("<manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>", xml_esc(p), xml_esc(ct)));
    }
    manifest_xml.push_str("</manifest:manifest>");

    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let werr = |e: zip::result::ZipError| FormatError::Write(e.to_string());
        let ioerr = |e: std::io::Error| FormatError::Write(e.to_string());

        // ODF 1.2 Part 3 §3.3: `mimetype` MUST be the first entry and MUST be stored uncompressed.
        z.start_file("mimetype", stored).map_err(werr)?;
        z.write_all(ODP_MIMETYPE.as_bytes()).map_err(ioerr)?;

        z.start_file("META-INF/manifest.xml", deflate).map_err(werr)?;
        z.write_all(manifest_xml.as_bytes()).map_err(ioerr)?;

        z.start_file("meta.xml", deflate).map_err(werr)?;
        z.write_all(meta_xml.as_bytes()).map_err(ioerr)?;

        z.start_file("styles.xml", deflate).map_err(werr)?;
        z.write_all(styles_xml.as_bytes()).map_err(ioerr)?;

        z.start_file("content.xml", deflate).map_err(werr)?;
        z.write_all(content_xml.as_bytes()).map_err(ioerr)?;

        for item in &pres.media {
            if let Some(path) = media_paths.get(&item.id) {
                z.start_file(path.as_str(), stored).map_err(werr)?;
                z.write_all(&item.data).map_err(ioerr)?;
            }
        }
        z.finish().map_err(werr)?;
    }
    Ok(buf.into_inner())
}
