//! Built-in searchable procedural vector icon library, canvas insertion, Icon Sheet generator
//! (`IconSheet.aip`), and multi-size batch SVG/PNG exporter (`icon.list`, `icon.insert`,
//! `icon.generateSheet`, `icon.exportBatch` — Illustrator 2027 `31.0` `IconSheet` & `iconspanel`).

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{
    Appearance, AppearanceItem, Artboard, CharStyle, Document, FillLayer, LineCap, LineJoin, Node, NodeKind, StrokeLayer, TextObject,
};
use vectorcraft_geom::{Affine, BezPath, PathData, Point, Rect, Vec2, shapes};
use vectorcraft_render::encode::{RasterExportOptions, RasterFormat};

use super::{CommandSpec, Result, Session, always, bad, cmd, color_value, has_doc, number_or, str_param, switch_or, text_param, unique_name};

pub const ICON_CATEGORIES: &[&str] = &["UI", "Arrows", "Media", "Files", "Commerce", "Badges"];
pub const DEFAULT_BATCH_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128];

#[derive(Clone, Copy, Debug)]
pub struct BuiltinIcon {
    pub name: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub tags: &'static [&'static str],
}

pub const CATALOG: &[BuiltinIcon] = &[
    // UI (6)
    BuiltinIcon { name: "home", label: "Home", category: "UI", tags: &["house", "start", "main", "dashboard"] },
    BuiltinIcon { name: "search", label: "Search", category: "UI", tags: &["find", "magnifier", "lookup", "lens"] },
    BuiltinIcon { name: "settings", label: "Settings", category: "UI", tags: &["gear", "preferences", "config", "options"] },
    BuiltinIcon { name: "user", label: "User", category: "UI", tags: &["account", "profile", "person", "avatar"] },
    BuiltinIcon { name: "bell", label: "Bell", category: "UI", tags: &["notification", "alert", "alarm", "ring"] },
    BuiltinIcon { name: "menu", label: "Menu", category: "UI", tags: &["hamburger", "bars", "navigation", "list"] },
    // Arrows (6)
    BuiltinIcon { name: "arrow-up", label: "Arrow Up", category: "Arrows", tags: &["north", "up", "top", "direction"] },
    BuiltinIcon { name: "arrow-down", label: "Arrow Down", category: "Arrows", tags: &["south", "down", "bottom", "direction"] },
    BuiltinIcon { name: "arrow-left", label: "Arrow Left", category: "Arrows", tags: &["west", "back", "previous", "left"] },
    BuiltinIcon { name: "arrow-right", label: "Arrow Right", category: "Arrows", tags: &["east", "next", "forward", "right"] },
    BuiltinIcon { name: "download", label: "Download", category: "Arrows", tags: &["save", "export", "inbox", "arrow"] },
    BuiltinIcon { name: "upload", label: "Upload", category: "Arrows", tags: &["publish", "share", "outbox", "arrow"] },
    // Media (6)
    BuiltinIcon { name: "play", label: "Play", category: "Media", tags: &["video", "audio", "start", "triangle"] },
    BuiltinIcon { name: "pause", label: "Pause", category: "Media", tags: &["hold", "stop", "audio", "bars"] },
    BuiltinIcon { name: "volume", label: "Volume", category: "Media", tags: &["speaker", "sound", "audio", "music"] },
    BuiltinIcon { name: "camera", label: "Camera", category: "Media", tags: &["photo", "snapshot", "capture", "lens"] },
    BuiltinIcon { name: "image", label: "Image", category: "Media", tags: &["picture", "gallery", "photo", "landscape"] },
    BuiltinIcon { name: "mic", label: "Microphone", category: "Media", tags: &["voice", "record", "podcast", "audio"] },
    // Files (6)
    BuiltinIcon { name: "file", label: "File", category: "Files", tags: &["document", "page", "paper", "sheet"] },
    BuiltinIcon { name: "folder", label: "Folder", category: "Files", tags: &["directory", "archive", "project", "storage"] },
    BuiltinIcon { name: "clipboard", label: "Clipboard", category: "Files", tags: &["paste", "task", "notes", "board"] },
    BuiltinIcon { name: "code", label: "Code", category: "Files", tags: &["brackets", "developer", "script", "source"] },
    BuiltinIcon { name: "calendar", label: "Calendar", category: "Files", tags: &["date", "schedule", "event", "month"] },
    BuiltinIcon { name: "clock", label: "Clock", category: "Files", tags: &["time", "history", "timer", "watch"] },
    // Commerce (6)
    BuiltinIcon { name: "cart", label: "Shopping Cart", category: "Commerce", tags: &["shop", "buy", "checkout", "store"] },
    BuiltinIcon { name: "tag", label: "Price Tag", category: "Commerce", tags: &["label", "discount", "sale", "coupon"] },
    BuiltinIcon { name: "credit-card", label: "Credit Card", category: "Commerce", tags: &["payment", "wallet", "billing", "card"] },
    BuiltinIcon { name: "gift", label: "Gift", category: "Commerce", tags: &["present", "reward", "box", "bonus"] },
    BuiltinIcon { name: "globe", label: "Globe", category: "Commerce", tags: &["world", "web", "language", "network"] },
    BuiltinIcon { name: "mail", label: "Mail", category: "Commerce", tags: &["email", "envelope", "message", "letter"] },
    // Badges (6)
    BuiltinIcon { name: "check", label: "Check", category: "Badges", tags: &["done", "tick", "success", "approve"] },
    BuiltinIcon { name: "plus", label: "Plus", category: "Badges", tags: &["add", "new", "create", "positive"] },
    BuiltinIcon { name: "close", label: "Close", category: "Badges", tags: &["cross", "cancel", "remove", "delete"] },
    BuiltinIcon { name: "heart", label: "Heart", category: "Badges", tags: &["like", "favorite", "love", "health"] },
    BuiltinIcon { name: "star", label: "Star", category: "Badges", tags: &["rating", "bookmark", "featured", "award"] },
    BuiltinIcon { name: "shield", label: "Shield", category: "Badges", tags: &["security", "verified", "protect", "guard"] },
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "icon.list",
            "List Icons",
            [],
            None,
            "{query?: string, category?: string} list built-in procedural vector icons and categories → {icons, categories, total}",
            always,
            run_list
        ),
        cmd!(
            "icon.insert",
            "Insert Icon",
            [],
            None,
            "{name?: string, size?: pt=24, x?: pt, y?: pt, strokeWidth?: pt=1.75, color?: #rrggbb} insert a built-in vector icon onto the active artboard → {id, name}",
            has_doc,
            run_insert
        ),
        cmd!(
            "icon.generateSheet",
            "Generate Icon Sheet…",
            ["Object"],
            None,
            "{icons?: [string…], category?: string, query?: string, columns?: int=6, cellSize?: pt=64, padding?: pt=20, iconSize?: pt=24, strokeWidth?: pt=1.75, color?: #rrggbb, showLabels?: bool=true, showGrid?: bool=true, createArtboard?: bool=true} generate a formatted vector icon sheet on the canvas → {id, count, columns, rows, artboard?}",
            has_doc,
            run_generate_sheet
        ),
        cmd!(
            query "icon.exportBatch",
            "Batch Export Icons",
            [],
            None,
            "{dir: string, icons?: [string…], category?: string, sizes?: [int…], formats?: [string…], color?: #rrggbb, strokeWidth?: pt=1.75} batch-export multi-size SVG and PNG icons (default sizes 16, 24, 32, 48, 64, 128) → {files, count, sizes, formats}",
            always,
            run_export_batch
        ),
    ]
}

pub fn matching_icons(query: Option<&str>, category: Option<&str>) -> Vec<&'static BuiltinIcon> {
    let q = query.map(str::trim).filter(|s| !s.is_empty()).map(str::to_ascii_lowercase);
    let cat = category.map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all")).map(str::to_ascii_lowercase);
    CATALOG
        .iter()
        .filter(|ic| {
            let cat_ok = cat.as_ref().is_none_or(|c| ic.category.to_ascii_lowercase() == *c);
            let q_ok = q.as_ref().is_none_or(|needle| {
                ic.name.contains(needle)
                    || ic.label.to_ascii_lowercase().contains(needle)
                    || ic.category.to_ascii_lowercase().contains(needle)
                    || ic.tags.iter().any(|t| t.contains(needle))
            });
            cat_ok && q_ok
        })
        .collect()
}

fn circle_bez(cx: f64, cy: f64, r: f64) -> BezPath {
    shapes::ellipse(Rect::new(cx - r, cy - r, cx + r, cy + r)).to_bezpath()
}

fn rect_bez(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    shapes::rectangle(Rect::new(x0, y0, x1, y1)).to_bezpath()
}

fn line_seg(bp: &mut BezPath, x0: f64, y0: f64, x1: f64, y1: f64) {
    bp.move_to(Point::new(x0, y0));
    bp.line_to(Point::new(x1, y1));
}

fn poly_open(bp: &mut BezPath, pts: &[(f64, f64)]) {
    if let Some(&(x0, y0)) = pts.first() {
        bp.move_to(Point::new(x0, y0));
        for &(x, y) in &pts[1..] {
            bp.line_to(Point::new(x, y));
        }
    }
}

fn poly_closed(bp: &mut BezPath, pts: &[(f64, f64)]) {
    poly_open(bp, pts);
    if !pts.is_empty() {
        bp.close_path();
    }
}

/// Returns a crisp vector `PathData` in `0..=24` × `0..=24` icon coordinates.
pub fn icon_path(name: &str) -> Option<PathData> {
    let mut bp = BezPath::new();
    match name {
        "home" => {
            poly_open(&mut bp, &[(3.0, 10.5), (12.0, 3.0), (21.0, 10.5)]);
            poly_open(&mut bp, &[(5.5, 9.5), (5.5, 20.5), (18.5, 20.5), (18.5, 9.5)]);
            poly_open(&mut bp, &[(10.0, 20.5), (10.0, 14.0), (14.0, 14.0), (14.0, 20.5)]);
        }
        "search" => {
            bp.extend(circle_bez(10.5, 10.5, 6.5));
            line_seg(&mut bp, 15.3, 15.3, 20.5, 20.5);
        }
        "settings" => {
            bp.extend(circle_bez(12.0, 12.0, 3.5));
            bp.extend(circle_bez(12.0, 12.0, 7.2));
            for i in 0..8 {
                let a = (i as f64) * std::f64::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                line_seg(&mut bp, 12.0 + 7.2 * c, 12.0 + 7.2 * s, 12.0 + 9.6 * c, 12.0 + 9.6 * s);
            }
        }
        "user" => {
            bp.extend(circle_bez(12.0, 8.0, 4.0));
            bp.move_to(Point::new(4.5, 20.5));
            bp.curve_to(Point::new(4.5, 15.5), Point::new(19.5, 15.5), Point::new(19.5, 20.5));
        }
        "bell" => {
            bp.move_to(Point::new(6.0, 16.5));
            bp.line_to(Point::new(6.0, 11.0));
            bp.curve_to(Point::new(6.0, 6.0), Point::new(18.0, 6.0), Point::new(18.0, 11.0));
            bp.line_to(Point::new(18.0, 16.5));
            line_seg(&mut bp, 4.0, 16.5, 20.0, 16.5);
            poly_open(&mut bp, &[(10.0, 19.5), (12.0, 21.0), (14.0, 19.5)]);
        }
        "menu" => {
            line_seg(&mut bp, 4.0, 7.0, 20.0, 7.0);
            line_seg(&mut bp, 4.0, 12.0, 20.0, 12.0);
            line_seg(&mut bp, 4.0, 17.0, 20.0, 17.0);
        }
        "arrow-up" => {
            line_seg(&mut bp, 12.0, 19.5, 12.0, 4.5);
            poly_open(&mut bp, &[(6.0, 10.5), (12.0, 4.5), (18.0, 10.5)]);
        }
        "arrow-down" => {
            line_seg(&mut bp, 12.0, 4.5, 12.0, 19.5);
            poly_open(&mut bp, &[(6.0, 13.5), (12.0, 19.5), (18.0, 13.5)]);
        }
        "arrow-left" => {
            line_seg(&mut bp, 19.5, 12.0, 4.5, 12.0);
            poly_open(&mut bp, &[(10.5, 6.0), (4.5, 12.0), (10.5, 18.0)]);
        }
        "arrow-right" => {
            line_seg(&mut bp, 4.5, 12.0, 19.5, 12.0);
            poly_open(&mut bp, &[(13.5, 6.0), (19.5, 12.0), (13.5, 18.0)]);
        }
        "download" => {
            line_seg(&mut bp, 12.0, 3.5, 12.0, 15.0);
            poly_open(&mut bp, &[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
            poly_open(&mut bp, &[(4.0, 15.5), (4.0, 20.0), (20.0, 20.0), (20.0, 15.5)]);
        }
        "upload" => {
            line_seg(&mut bp, 12.0, 15.5, 12.0, 4.0);
            poly_open(&mut bp, &[(7.5, 8.5), (12.0, 4.0), (16.5, 8.5)]);
            poly_open(&mut bp, &[(4.0, 15.5), (4.0, 20.0), (20.0, 20.0), (20.0, 15.5)]);
        }
        "play" => {
            poly_closed(&mut bp, &[(6.5, 4.5), (19.5, 12.0), (6.5, 19.5)]);
        }
        "pause" => {
            bp.extend(rect_bez(6.0, 4.5, 10.0, 19.5));
            bp.extend(rect_bez(14.0, 4.5, 18.0, 19.5));
        }
        "volume" => {
            poly_closed(&mut bp, &[(4.0, 9.5), (8.5, 9.5), (13.5, 5.5), (13.5, 18.5), (8.5, 14.5), (4.0, 14.5)]);
            bp.move_to(Point::new(16.5, 8.5));
            bp.curve_to(Point::new(18.5, 10.2), Point::new(18.5, 13.8), Point::new(16.5, 15.5));
            bp.move_to(Point::new(18.8, 6.0));
            bp.curve_to(Point::new(21.8, 9.2), Point::new(21.8, 14.8), Point::new(18.8, 18.0));
        }
        "camera" => {
            poly_closed(&mut bp, &[(3.0, 7.5), (7.0, 7.5), (8.8, 5.0), (15.2, 5.0), (17.0, 7.5), (21.0, 7.5), (21.0, 19.5), (3.0, 19.5)]);
            bp.extend(circle_bez(12.0, 13.5, 3.6));
        }
        "image" => {
            bp.extend(rect_bez(3.5, 4.5, 20.5, 19.5));
            bp.extend(circle_bez(8.5, 9.5, 1.8));
            poly_open(&mut bp, &[(4.0, 18.5), (10.5, 12.5), (14.5, 16.0), (17.5, 13.5), (20.5, 17.0)]);
        }
        "mic" => {
            bp.extend(shapes::rounded_rectangle(Rect::new(9.0, 3.0, 15.0, 13.5), 3.0).to_bezpath());
            bp.move_to(Point::new(5.5, 11.0));
            bp.curve_to(Point::new(5.5, 18.0), Point::new(18.5, 18.0), Point::new(18.5, 11.0));
            line_seg(&mut bp, 12.0, 17.0, 12.0, 21.0);
            line_seg(&mut bp, 8.5, 21.0, 15.5, 21.0);
        }
        "file" => {
            poly_closed(&mut bp, &[(5.5, 3.0), (14.5, 3.0), (18.5, 7.0), (18.5, 21.0), (5.5, 21.0)]);
            poly_open(&mut bp, &[(14.0, 3.0), (14.0, 7.5), (18.5, 7.5)]);
        }
        "folder" => {
            poly_closed(&mut bp, &[(3.0, 5.5), (9.5, 5.5), (11.5, 8.0), (21.0, 8.0), (21.0, 19.5), (3.0, 19.5)]);
        }
        "clipboard" => {
            bp.extend(rect_bez(5.5, 5.5, 18.5, 21.0));
            bp.extend(rect_bez(9.0, 3.0, 15.0, 7.0));
            line_seg(&mut bp, 8.5, 11.5, 15.5, 11.5);
            line_seg(&mut bp, 8.5, 15.5, 15.5, 15.5);
        }
        "code" => {
            poly_open(&mut bp, &[(8.5, 7.0), (3.5, 12.0), (8.5, 17.0)]);
            poly_open(&mut bp, &[(15.5, 7.0), (20.5, 12.0), (15.5, 17.0)]);
            line_seg(&mut bp, 13.5, 5.0, 10.5, 19.0);
        }
        "calendar" => {
            bp.extend(rect_bez(3.5, 5.5, 20.5, 20.5));
            line_seg(&mut bp, 3.5, 10.0, 20.5, 10.0);
            line_seg(&mut bp, 8.0, 3.0, 8.0, 7.0);
            line_seg(&mut bp, 16.0, 3.0, 16.0, 7.0);
        }
        "clock" => {
            bp.extend(circle_bez(12.0, 12.0, 8.5));
            poly_open(&mut bp, &[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
        }
        "cart" => {
            poly_open(&mut bp, &[(2.5, 4.5), (5.5, 4.5), (8.0, 15.5), (18.5, 15.5), (20.5, 8.0), (6.5, 8.0)]);
            bp.extend(circle_bez(9.0, 19.0, 1.6));
            bp.extend(circle_bez(17.5, 19.0, 1.6));
        }
        "tag" => {
            poly_closed(&mut bp, &[(3.5, 12.0), (12.0, 3.5), (20.0, 3.5), (20.0, 11.5), (11.5, 20.0)]);
            bp.extend(circle_bez(16.0, 7.5, 1.4));
        }
        "credit-card" => {
            bp.extend(shapes::rounded_rectangle(Rect::new(2.5, 5.5, 21.5, 18.5), 2.0).to_bezpath());
            line_seg(&mut bp, 2.5, 10.0, 21.5, 10.0);
            line_seg(&mut bp, 6.0, 15.0, 11.0, 15.0);
        }
        "gift" => {
            bp.extend(rect_bez(4.5, 10.5, 19.5, 20.5));
            bp.extend(rect_bez(3.5, 7.0, 20.5, 10.5));
            line_seg(&mut bp, 12.0, 7.0, 12.0, 20.5);
            bp.extend(circle_bez(9.8, 5.2, 1.8));
            bp.extend(circle_bez(14.2, 5.2, 1.8));
        }
        "globe" => {
            bp.extend(circle_bez(12.0, 12.0, 8.5));
            line_seg(&mut bp, 3.5, 12.0, 20.5, 12.0);
            bp.extend(shapes::ellipse(Rect::new(8.2, 3.5, 15.8, 20.5)).to_bezpath());
        }
        "mail" => {
            bp.extend(rect_bez(3.0, 5.5, 21.0, 18.5));
            poly_open(&mut bp, &[(3.0, 6.5), (12.0, 13.0), (21.0, 6.5)]);
        }
        "check" => {
            poly_open(&mut bp, &[(4.5, 12.5), (9.5, 17.5), (19.5, 6.5)]);
        }
        "plus" => {
            line_seg(&mut bp, 12.0, 4.5, 12.0, 19.5);
            line_seg(&mut bp, 4.5, 12.0, 19.5, 12.0);
        }
        "close" => {
            line_seg(&mut bp, 6.0, 6.0, 18.0, 18.0);
            line_seg(&mut bp, 18.0, 6.0, 6.0, 18.0);
        }
        "heart" => {
            bp.move_to(Point::new(12.0, 20.0));
            bp.curve_to(Point::new(3.0, 14.5), Point::new(2.5, 6.0), Point::new(7.5, 5.0));
            bp.curve_to(Point::new(10.2, 4.5), Point::new(11.5, 6.5), Point::new(12.0, 8.0));
            bp.curve_to(Point::new(12.5, 6.5), Point::new(13.8, 4.5), Point::new(16.5, 5.0));
            bp.curve_to(Point::new(21.5, 6.0), Point::new(21.0, 14.5), Point::new(12.0, 20.0));
            bp.close_path();
        }
        "star" => {
            return Some(shapes::star(Point::new(12.0, 12.0), 9.2, 4.2, 5, -90.0));
        }
        "shield" => {
            bp.move_to(Point::new(12.0, 3.0));
            bp.line_to(Point::new(19.5, 6.0));
            bp.curve_to(Point::new(19.5, 13.5), Point::new(16.5, 18.0), Point::new(12.0, 21.0));
            bp.curve_to(Point::new(7.5, 18.0), Point::new(4.5, 13.5), Point::new(4.5, 6.0));
            bp.close_path();
        }
        _ => return None,
    }
    Some(PathData::from_bezpath(&bp))
}

fn make_icon_node(id: vectorcraft_doc::NodeId, name: &str, top_left: Point, size: f64, stroke_w: f64, color: Color) -> Result<Node> {
    let base = icon_path(name).ok_or_else(|| bad("icon.insert", format!("unknown built-in icon `{name}`")))?;
    let scale = size / 24.0;
    let xf = Affine::translate(Vec2::new(top_left.x, top_left.y)) * Affine::scale(scale);
    let path = base.transformed(xf);
    let mut stroke = StrokeLayer::new(Paint::solid(color), (stroke_w * scale).max(0.5));
    stroke.cap = LineCap::Round;
    stroke.join = LineJoin::Round;
    let app = Appearance { items: vec![AppearanceItem::Stroke(stroke)], ..Appearance::default() };
    let mut node = Node::path(id, path, app);
    node.name = Some(format!("Icon: {name}"));
    Ok(node)
}

fn parse_stroke_color(p: &Value, cmd: &str) -> Result<Color> {
    match p.get("color") {
        None | Some(Value::Null) => Ok(Color::rgb(0.14, 0.16, 0.2)),
        Some(v) => color_value(v).ok_or_else(|| bad(cmd, "invalid `color`")),
    }
}

fn resolve_icon_names(p: &Value, cmd: &str) -> Result<Vec<&'static BuiltinIcon>> {
    if let Some(arr) = p.get("icons").filter(|v| !v.is_null()) {
        let list = arr.as_array().ok_or_else(|| bad(cmd, "`icons` must be an array of icon names"))?;
        let mut out = Vec::with_capacity(list.len());
        for item in list {
            let name = item.as_str().ok_or_else(|| bad(cmd, "each item in `icons` must be a string"))?;
            let found = CATALOG.iter().find(|ic| ic.name == name).ok_or_else(|| bad(cmd, format!("unknown built-in icon `{name}`")))?;
            out.push(found);
        }
        if out.is_empty() {
            return Err(bad(cmd, "`icons` cannot be empty"));
        }
        return Ok(out);
    }
    let q = text_param(p, "query", cmd)?;
    let cat = text_param(p, "category", cmd)?;
    let out = matching_icons(q, cat);
    if out.is_empty() {
        return Err(bad(cmd, "no built-in icons match the given filter"));
    }
    Ok(out)
}

fn run_list(_s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "icon.list";
    let q = text_param(p, "query", C)?;
    let cat = text_param(p, "category", C)?;
    let list: Vec<Value> =
        matching_icons(q, cat).into_iter().map(|ic| json!({"name": ic.name, "label": ic.label, "category": ic.category, "tags": ic.tags})).collect();
    Ok(json!({
        "icons": list,
        "categories": ICON_CATEGORIES,
        "total": CATALOG.len(),
    }))
}

fn target_layer(d: &Document, sel: &vectorcraft_doc::Selection) -> Option<vectorcraft_doc::NodeId> {
    let from_sel = sel
        .objects
        .first()
        .and_then(|&id| d.ancestry(id))
        .and_then(|a| a.first().copied())
        .filter(|&lid| d.node(lid).is_some_and(|l| l.visible && !l.locked && !l.is_template()));
    from_sel.or_else(|| d.layers.iter().rev().find(|l| l.visible && !l.locked && !l.is_template()).map(|l| l.id))
}

fn run_insert(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "icon.insert";
    let name = text_param(p, "name", C)?.unwrap_or("home");
    if icon_path(name).is_none() {
        return Err(bad(C, format!("unknown built-in icon `{name}`")));
    }
    let size = number_or(p, "size", 24.0, C)?;
    if !size.is_finite() || !(4.0..=2048.0).contains(&size) {
        return Err(bad(C, "`size` must be between 4 and 2048 pt"));
    }
    let stroke_w = number_or(p, "strokeWidth", 1.75, C)?;
    if !stroke_w.is_finite() || !(0.25..=24.0).contains(&stroke_w) {
        return Err(bad(C, "`strokeWidth` must be between 0.25 and 24 pt"));
    }
    let color = parse_stroke_color(p, C)?;
    let ab_center = s.doc()?.doc.artboards.first().map_or(Point::new(200.0, 200.0), |a| a.rect.center());
    let cx = number_or(p, "x", ab_center.x, C)?;
    let cy = number_or(p, "y", ab_center.y, C)?;
    if !cx.is_finite() || !cy.is_finite() {
        return Err(bad(C, "icon coordinates must be finite"));
    }
    let top_left = Point::new(cx - size * 0.5, cy - size * 0.5);

    let id = s.edit("Insert Icon", |d, sel| {
        let lid = target_layer(d, sel).ok_or_else(|| bad(C, "no editable layer"))?;
        let nid = d.alloc_id();
        let node = make_icon_node(nid, name, top_left, size, stroke_w, color)?;
        d.insert(Some(lid), usize::MAX, node)?;
        sel.set([nid]);
        Ok(nid)
    })?;
    Ok(json!({"id": id.0, "name": name}))
}

#[derive(Clone, Copy)]
struct SheetLayout {
    columns: usize,
    cell_size: f64,
    padding: f64,
    icon_size: f64,
    stroke_w: f64,
    show_labels: bool,
    show_grid: bool,
    create_artboard: bool,
}

fn parse_sheet_layout(p: &Value) -> Result<SheetLayout> {
    const C: &str = "icon.generateSheet";
    let cols_f = number_or(p, "columns", 6.0, C)?;
    if !cols_f.is_finite() || !(1.0..=24.0).contains(&cols_f) || cols_f.fract() != 0.0 {
        return Err(bad(C, "`columns` must be an integer between 1 and 24"));
    }
    let cell_size = number_or(p, "cellSize", 64.0, C)?;
    if !cell_size.is_finite() || !(16.0..=512.0).contains(&cell_size) {
        return Err(bad(C, "`cellSize` must be between 16 and 512 pt"));
    }
    let padding = number_or(p, "padding", 20.0, C)?;
    if !padding.is_finite() || !(0.0..=256.0).contains(&padding) {
        return Err(bad(C, "`padding` must be between 0 and 256 pt"));
    }
    let icon_size = number_or(p, "iconSize", 24.0, C)?;
    if !icon_size.is_finite() || !(8.0..=512.0).contains(&icon_size) {
        return Err(bad(C, "`iconSize` must be between 8 and 512 pt"));
    }
    let stroke_w = number_or(p, "strokeWidth", 1.75, C)?;
    if !stroke_w.is_finite() || !(0.25..=16.0).contains(&stroke_w) {
        return Err(bad(C, "`strokeWidth` must be between 0.25 and 16 pt"));
    }
    Ok(SheetLayout {
        columns: cols_f as usize,
        cell_size,
        padding,
        icon_size,
        stroke_w,
        show_labels: switch_or(p, "showLabels", true, C)?,
        show_grid: switch_or(p, "showGrid", true, C)?,
        create_artboard: switch_or(p, "createArtboard", true, C)?,
    })
}

fn build_sheet_cell(d: &mut Document, ic: &BuiltinIcon, cell_rect: Rect, lay: SheetLayout, color: Color) -> Result<Arc<Node>> {
    let mut items = Vec::with_capacity(3);
    if lay.show_grid {
        let card = shapes::rounded_rectangle(cell_rect.inflate(-2.0, -2.0), 6.0);
        let app = Appearance {
            items: vec![
                AppearanceItem::Fill(FillLayer::new(Paint::solid(Color::rgb(0.97, 0.975, 0.985)))),
                AppearanceItem::Stroke(StrokeLayer::new(Paint::solid(Color::rgb(0.86, 0.88, 0.91)), 0.75)),
            ],
            ..Appearance::default()
        };
        items.push(Arc::new(Node::path(d.alloc_id(), card, app)));
    }
    let icon_y_offset = if lay.show_labels { -6.0 } else { 0.0 };
    let top_left = Point::new(cell_rect.center().x - lay.icon_size * 0.5, cell_rect.center().y - lay.icon_size * 0.5 + icon_y_offset);
    items.push(Arc::new(make_icon_node(d.alloc_id(), ic.name, top_left, lay.icon_size, lay.stroke_w, color)?));

    if lay.show_labels {
        let style = CharStyle { size: 8.0, fill: Paint::solid(Color::rgb(0.36, 0.39, 0.45)), ..CharStyle::default() };
        let mut txt = TextObject::point(Point::new(cell_rect.center().x, cell_rect.y1 - 8.0), ic.name, style);
        txt.para.justify = vectorcraft_doc::Justify::Center;
        super::typecmd::refresh_bounds(&mut txt);
        items.push(Arc::new(Node::new(d.alloc_id(), NodeKind::Text(Box::new(txt)))));
    }
    let mut g = Node::group(d.alloc_id(), items);
    g.name = Some(ic.name.to_string());
    Ok(Arc::new(g))
}

fn run_generate_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "icon.generateSheet";
    let icons = resolve_icon_names(p, C)?;
    let lay = parse_sheet_layout(p)?;
    let color = parse_stroke_color(p, C)?;
    let rows = icons.len().div_ceil(lay.columns);
    let sheet_w = lay.padding * 2.0 + (lay.columns as f64) * lay.cell_size;
    let sheet_h = lay.padding * 2.0 + (rows as f64) * lay.cell_size;

    let origin_x = s.doc()?.doc.artboards.iter().map(|a| a.rect.x1).fold(0.0_f64, f64::max) + if lay.create_artboard { 40.0 } else { 0.0 };
    let origin_y = s.doc()?.doc.artboards.first().map_or(0.0, |a| a.rect.y0);
    let sheet_rect = Rect::new(origin_x, origin_y, origin_x + sheet_w, origin_y + sheet_h);

    let (gid, ab_idx) = s.edit("Generate Icon Sheet", |d, sel| {
        let lid = target_layer(d, sel).ok_or_else(|| bad(C, "no editable layer"))?;
        let mut cells = Vec::with_capacity(icons.len());
        for (idx, ic) in icons.iter().enumerate() {
            let col = idx % lay.columns;
            let row = idx / lay.columns;
            let x0 = sheet_rect.x0 + lay.padding + (col as f64) * lay.cell_size;
            let y0 = sheet_rect.y0 + lay.padding + (row as f64) * lay.cell_size;
            let cell_rect = Rect::new(x0, y0, x0 + lay.cell_size, y0 + lay.cell_size);
            cells.push(build_sheet_cell(d, ic, cell_rect, lay, color)?);
        }
        let gid = d.alloc_id();
        let mut group = Node::group(gid, cells);
        group.name = Some("Icon Sheet".into());
        d.insert(Some(lid), usize::MAX, group)?;

        let ab = if lay.create_artboard {
            let ab_id = d.next_artboard_id();
            let ab_name = unique_name("Icon Sheet", |cand| d.artboards.iter().any(|a| a.name == cand));
            d.artboards.push(Artboard {
                id: ab_id,
                name: ab_name,
                rect: sheet_rect,
                locked: false,
                show_center_mark: false,
                show_cross_hairs: false,
                show_video_safe_areas: false,
                video_ruler_pixel_aspect_ratio: 1.0,
                color: Some(Color::WHITE),
                guide_label: None,
            });
            Some(d.artboards.len() - 1)
        } else {
            None
        };
        sel.set([gid]);
        Ok((gid, ab))
    })?;

    let mut out = json!({
        "id": gid.0,
        "count": icons.len(),
        "columns": lay.columns,
        "rows": rows,
    });
    if let Some(ab) = ab_idx {
        out["artboard"] = json!(ab);
    }
    Ok(out)
}

fn parse_batch_sizes(p: &Value, cmd: &str) -> Result<Vec<u32>> {
    let Some(v) = p.get("sizes").filter(|v| !v.is_null()) else {
        return Ok(DEFAULT_BATCH_SIZES.to_vec());
    };
    let arr = v.as_array().ok_or_else(|| bad(cmd, "`sizes` must be an array of pixel sizes"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item.as_u64().ok_or_else(|| bad(cmd, "each size in `sizes` must be a positive integer"))?;
        if !(8..=1024).contains(&n) {
            return Err(bad(cmd, "icon export sizes must be between 8 and 1024 px"));
        }
        out.push(n as u32);
    }
    if out.is_empty() {
        return Err(bad(cmd, "`sizes` cannot be empty"));
    }
    Ok(out)
}

fn parse_batch_formats(p: &Value, cmd: &str) -> Result<(bool, bool)> {
    let Some(v) = p.get("formats").filter(|v| !v.is_null()) else {
        return Ok((true, true));
    };
    let arr = v.as_array().ok_or_else(|| bad(cmd, "`formats` must be an array (\"svg\", \"png\")"))?;
    let (mut svg, mut png) = (false, false);
    for item in arr {
        match item.as_str().unwrap_or("") {
            s if s.eq_ignore_ascii_case("svg") => svg = true,
            s if s.eq_ignore_ascii_case("png") => png = true,
            other => return Err(bad(cmd, format!("unsupported batch icon format `{other}` (expected svg or png)"))),
        }
    }
    if !svg && !png {
        return Err(bad(cmd, "`formats` must include at least one of \"svg\" or \"png\""));
    }
    Ok((svg, png))
}

fn single_icon_doc(name: &str, size_px: u32, stroke_w: f64, color: Color) -> Result<Document> {
    let s = f64::from(size_px);
    let mut d = Document::new(s, s);
    let nid = d.alloc_id();
    let node = make_icon_node(nid, name, Point::ZERO, s, stroke_w, color)?;
    if let Some(lid) = d.layers.first().map(|l| l.id) {
        let _ = d.insert(Some(lid), usize::MAX, node);
    }
    Ok(d)
}

fn run_export_batch(_s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "icon.exportBatch";
    let dir = str_param(p, "dir").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(C, "missing string `dir`"))?;
    let icons = resolve_icon_names(p, C)?;
    let sizes = parse_batch_sizes(p, C)?;
    let (want_svg, want_png) = parse_batch_formats(p, C)?;
    let stroke_w = number_or(p, "strokeWidth", 1.75, C)?;
    if !stroke_w.is_finite() || !(0.25..=16.0).contains(&stroke_w) {
        return Err(bad(C, "`strokeWidth` must be between 0.25 and 16 pt"));
    }
    let color = parse_stroke_color(p, C)?;

    super::fileio::create_dir(dir)?;
    let mut renderer = vectorcraft_render::Renderer::new();
    let mut written = Vec::new();

    for &sz in &sizes {
        let sub_dir = std::path::Path::new(dir).join(format!("{sz}x{sz}"));
        let sub_str = sub_dir.to_string_lossy().into_owned();
        super::fileio::create_dir(&sub_str)?;
        let region = Rect::new(0.0, 0.0, f64::from(sz), f64::from(sz));
        let png_opts = RasterExportOptions { ppi: 72.0, background: None, ..RasterExportOptions::default() };

        for ic in &icons {
            let doc = single_icon_doc(ic.name, sz, stroke_w, color)?;
            if want_svg {
                let svg_text = vectorcraft_svg::export(
                    &doc,
                    &vectorcraft_svg::ExportOptions { artboard: Some(0), object_ids: vectorcraft_svg::ObjectIds::Minimal, ..Default::default() },
                );
                let out_path = sub_dir.join(format!("{}.svg", ic.name)).to_string_lossy().into_owned();
                super::fileio::write_file(&out_path, svg_text.as_bytes())?;
                written.push(out_path);
            }
            if want_png {
                let png_bytes = renderer.export_region(&doc, region, RasterFormat::Png, &png_opts).map_err(crate::EngineError::Other)?;
                let out_path = sub_dir.join(format!("{}.png", ic.name)).to_string_lossy().into_owned();
                super::fileio::write_file(&out_path, &png_bytes)?;
                written.push(out_path);
            }
        }
    }

    let mut fmts = Vec::new();
    if want_svg {
        fmts.push("svg");
    }
    if want_png {
        fmts.push("png");
    }
    Ok(json!({
        "count": written.len(),
        "files": written,
        "sizes": sizes,
        "formats": fmts,
    }))
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use serde_json::json;

    #[test]
    fn icon_catalog_insert_sheet_and_multi_size_batch_export_work() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();

        let all = s.execute("icon.list", &json!({})).unwrap();
        assert_eq!(all["total"], 36);
        let arrows = s.execute("icon.list", &json!({"category": "Arrows"})).unwrap();
        assert_eq!(arrows["icons"].as_array().unwrap().len(), 6);

        let ins = s.execute("icon.insert", &json!({"name": "camera", "size": 32, "x": 100, "y": 100})).unwrap();
        assert_eq!(ins["name"], "camera");
        let cid = vectorcraft_doc::NodeId(ins["id"].as_u64().unwrap());
        assert!(s.active().unwrap().doc.node(cid).is_some());
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(s.active().unwrap().doc.node(cid).is_none());

        let sheet = s.execute("icon.generateSheet", &json!({"category": "UI", "columns": 3, "cellSize": 56, "createArtboard": true})).unwrap();
        assert_eq!(sheet["count"], 6);
        assert_eq!(sheet["columns"], 3);
        assert_eq!(sheet["rows"], 2);
        assert_eq!(sheet["artboard"], 1);

        let tmp_dir = std::env::temp_dir().join(format!("vc_icon_batch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let batch = s
            .execute(
                "icon.exportBatch",
                &json!({
                    "dir": tmp_dir.to_string_lossy(),
                    "icons": ["home", "star"],
                    "sizes": [16, 24, 32, 48, 64, 128],
                    "formats": ["svg", "png"]
                }),
            )
            .unwrap();
        assert_eq!(batch["count"], 2 * 6 * 2);
        assert!(tmp_dir.join("16x16").join("home.svg").is_file());
        assert!(tmp_dir.join("128x128").join("star.png").is_file());
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
