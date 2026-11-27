//! Adobe Capture (`FFC_capture`) Asset Extractor commands:
//! - `capture.extractColorTheme`: Extracts a 5-swatch harmonious OKLab color theme (`Colorful`,
//!   `Bright`, `Muted`, `Deep`, `Dark` moods) from raster images and/or vector artwork, adds it as
//!   a document `SwatchGroup` in one undo step, and optionally saves it to `Prefs::color_themes`.
//! - `capture.extractPatternTile`: Generates a seamless `Kaleidoscope`, `Mirror`, or `Half-Drop`
//!   vector/raster pattern tile from selected artwork and adds it to the document's patterns & swatches.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::capture::{CAPTURE_THEME_SWATCHES, CaptureMood, extract_color_theme_colors, extract_color_theme_rgba};
use vectorcraft_color::{Color, Paint, Swatch, SwatchGroup};
use vectorcraft_doc::pattern::{PatternDef, TileType, pattern_paint, reflect_about};
use vectorcraft_doc::swatches::node_colors;
use vectorcraft_doc::{Appearance, ColorMode, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, BezPath, FillRule, PathData, Point, Rect, Vec2, shapes};

use super::colortheme::ColorTheme;
use super::edit::{outermost, selected_roots};
use super::patterncmds::add_pattern;
use super::{CommandSpec, EngineError, Result, Session, bad, cmd, given_objects, has_doc, has_selection, number_or, switch_or, text_param};

pub const CMD_COLOR_THEME: &str = "capture.extractColorTheme";
pub const CMD_PATTERN_TILE: &str = "capture.extractPatternTile";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapturePatternMode {
    #[default]
    Kaleidoscope,
    Mirror,
    HalfDrop,
}

impl CapturePatternMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Kaleidoscope => "kaleidoscope",
            Self::Mirror => "mirror",
            Self::HalfDrop => "halfDrop",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "kaleidoscope" | "만화경" => Some(Self::Kaleidoscope),
            "mirror" | "대칭거울" | "거울" => Some(Self::Mirror),
            "halfdrop" | "반드롭" => Some(Self::HalfDrop),
            _ => None,
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "capture.extractColorTheme",
            "Extract Color Theme…",
            ["Object", "Capture"],
            None,
            "{ids?|id?, mood?: \"colorful\"|\"bright\"|\"muted\"|\"deep\"|\"dark\" (\"colorful\"), name?: string, global?: bool=false, saveTheme?: bool=true} extract a 5-swatch harmonious OKLab color theme from raster/vector artwork and add it to Swatches → {group, mood, colors, swatches}",
            has_doc,
            run_extract_color_theme
        ),
        cmd!(
            "capture.extractPatternTile",
            "Extract Pattern Tile…",
            ["Object", "Capture"],
            None,
            "{ids?|id?, mode?: \"kaleidoscope\"|\"mirror\"|\"halfDrop\" (\"kaleidoscope\"), segments?: 4|6|8|12 (6), tileSize?: pt, name?: string, placePreview?: bool=false} generate a seamless kaleidoscope, mirror, or half-drop pattern tile from selected artwork → {name, mode, tileType, width, height, previewId?}",
            can_extract_pattern,
            run_extract_pattern_tile
        ),
    ]
}

fn can_extract_pattern(s: &Session) -> std::result::Result<(), String> {
    has_selection(s)?;
    super::maskedit::outside_mask_editing(s)?;
    if s.active().is_some_and(|st| st.doc.pattern_edit.is_some() || st.doc.symbol_edit.is_some()) {
        return Err("exit isolation mode first".into());
    }
    Ok(())
}

// ------------------------------------------------------------------ Color Theme Extraction

fn collect_node_samples(doc: &Document, roots: &[Arc<Node>], rgba_out: &mut Vec<[u8; 4]>, colors_out: &mut Vec<Color>) {
    let keep_all = |_: f64, _: f64| true;
    for root in roots {
        root.walk(&mut |n| {
            if let NodeKind::Image(img) = &n.kind {
                let blob = doc.images.get(&img.key).or(img.pixels.as_deref());
                if let Some(b) = blob
                    && let Some(mut picks) = b.sample_pixels(&[(None, 4096, &keep_all)])
                    && let Some(px) = picks.pop()
                {
                    rgba_out.extend(px);
                }
            }
        });
        node_colors(root, &mut |c, _| colors_out.push(*c));
    }
}

fn next_capture_group_name(doc: &Document, mood: CaptureMood) -> String {
    let base = format!("Capture {}", mood.label());
    doc.free_swatch_name(&base)
}

fn run_extract_color_theme(s: &mut Session, p: &Value) -> Result<Value> {
    let mood_str = text_param(p, "mood", CMD_COLOR_THEME)?.unwrap_or("colorful");
    let mood =
        CaptureMood::parse(mood_str).ok_or_else(|| bad(CMD_COLOR_THEME, format!("unknown mood `{mood_str}` (colorful|bright|muted|deep|dark)")))?;
    let wanted_name = text_param(p, "name", CMD_COLOR_THEME)?.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    let global = switch_or(p, "global", false, CMD_COLOR_THEME)?;
    let save_theme = switch_or(p, "saveTheme", true, CMD_COLOR_THEME)?;

    let explicit_ids = given_objects(s, p, CMD_COLOR_THEME)?;
    let st = s.doc()?;
    let target_nodes: Vec<Arc<Node>> = if let Some(ids) = explicit_ids {
        outermost(&st.doc, ids).into_iter().filter_map(|id| st.doc.node_arc(id).cloned()).collect()
    } else if !st.selection.objects.is_empty() {
        selected_roots(s)?.into_iter().filter_map(|id| st.doc.node_arc(id).cloned()).collect()
    } else {
        st.doc.layers.clone()
    };

    let mut rgba_samples = Vec::new();
    let mut vec_colors = Vec::new();
    collect_node_samples(&st.doc, &target_nodes, &mut rgba_samples, &mut vec_colors);

    let extracted: [Color; CAPTURE_THEME_SWATCHES] = if !rgba_samples.is_empty() {
        for c in &vec_colors {
            rgba_samples.push(c.to_rgba8(1.0));
        }
        extract_color_theme_rgba(&rgba_samples, mood)
    } else {
        extract_color_theme_colors(&vec_colors, mood)
    };

    let cmyk_doc = st.doc.color_mode == ColorMode::Cmyk;
    let hex_list: Vec<String> = extracted.iter().map(Color::to_hex).collect();
    let doc_colors: Vec<Color> = extracted
        .iter()
        .map(|c| {
            if cmyk_doc {
                let [cc, m, y, k] = c.to_cmyk();
                let pct = |v: f32| (v.clamp(0.0, 1.0) * 100.0).round() / 100.0;
                Color::cmyk(pct(cc), pct(m), pct(y), pct(k))
            } else {
                *c
            }
        })
        .collect();

    let (group_name, swatch_names) = s.edit("Extract Color Theme", |d, _| {
        let mut free = d.free_swatch_names();
        let gname = match &wanted_name {
            Some(n) => free.take(n),
            None => {
                let def_name = next_capture_group_name(d, mood);
                free.take(&def_name)
            }
        };
        let mut swatches = Vec::with_capacity(CAPTURE_THEME_SWATCHES);
        let mut names = Vec::with_capacity(CAPTURE_THEME_SWATCHES);
        for (i, &col) in doc_colors.iter().enumerate() {
            let paint = Paint::solid(col);
            let sname = free.take(&format!("{gname} {}", i + 1));
            names.push(sname.clone());
            swatches.push(Swatch { name: sname, paint, global, spot: false });
        }
        d.swatch_groups.push(SwatchGroup { name: gname.clone(), swatches });
        Ok((gname, names))
    })?;

    if save_theme {
        let theme_name = super::unique_name(&group_name, |cand| s.prefs.color_themes.iter().any(|t| t.name.eq_ignore_ascii_case(cand)));
        s.prefs.color_themes.push(ColorTheme { name: theme_name, colors: extracted.to_vec(), rule: Some(format!("capture:{}", mood.id())) });
    }

    Ok(json!({
        "group": group_name,
        "mood": mood.id(),
        "colors": hex_list,
        "swatches": swatch_names,
    }))
}

// ------------------------------------------------------------------ Pattern Tile Extraction

fn clip_path_node(id: NodeId, path: PathData) -> Arc<Node> {
    Arc::new(Node::new(id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: true, guide: false }))
}

fn clip_group_node(id: NodeId, children: Vec<Arc<Node>>) -> Arc<Node> {
    Arc::new(Node::new(id, NodeKind::Group { children, clip: true }))
}

fn wedge_polygon(center: Point, radius: f64, a0: f64, a1: f64) -> PathData {
    let mid = 0.5 * (a0 + a1);
    let r = radius * 1.45;
    let mut bp = BezPath::new();
    bp.move_to(center);
    bp.line_to(Point::new(center.x + r * a0.cos(), center.y + r * a0.sin()));
    bp.line_to(Point::new(center.x + r * mid.cos(), center.y + r * mid.sin()));
    bp.line_to(Point::new(center.x + r * a1.cos(), center.y + r * a1.sin()));
    bp.close_path();
    PathData::from_bezpath(&bp)
}

fn clone_transformed_group(d: &mut Document, src: &[Arc<Node>], xf: Affine) -> Arc<Node> {
    let children: Vec<Arc<Node>> = src
        .iter()
        .map(|n| {
            let mut copy = d.reid(n);
            copy.transform(xf, true);
            Arc::new(copy)
        })
        .collect();
    if children.len() == 1
        && let Some(only) = children.first().cloned()
    {
        return only;
    }
    Arc::new(Node::group(d.alloc_id(), children))
}

fn build_mirror_tile_art(d: &mut Document, norm_src: &[Arc<Node>], tile_rect: Rect) -> Vec<Arc<Node>> {
    let cx = tile_rect.center().x;
    let cy = tile_rect.center().y;
    let half_w = tile_rect.width() * 0.5;
    let half_h = tile_rect.height() * 0.5;
    // Scale normalized source into top-left quadrant, then reflect seamlessly into all 4 quadrants.
    let q_rect = Rect::new(tile_rect.x0, tile_rect.y0, cx, cy);
    let fit_q = Affine::translate(q_rect.origin().to_vec2())
        * Affine::scale_non_uniform(half_w / tile_rect.width().max(1e-3), half_h / tile_rect.height().max(1e-3))
        * Affine::translate(-tile_rect.origin().to_vec2());

    let quadrants: [(Rect, bool, bool); 4] = [
        (Rect::new(tile_rect.x0, tile_rect.y0, cx, cy), false, false),
        (Rect::new(cx, tile_rect.y0, tile_rect.x1, cy), true, false),
        (Rect::new(tile_rect.x0, cy, cx, tile_rect.y1), false, true),
        (Rect::new(cx, cy, tile_rect.x1, tile_rect.y1), true, true),
    ];

    let mut out = Vec::with_capacity(4);
    for (qr, flip_x, flip_y) in quadrants {
        let mut xf = fit_q;
        if flip_x {
            xf = reflect_about(Point::new(cx, cy), 90.0) * xf;
        }
        if flip_y {
            xf = reflect_about(Point::new(cx, cy), 0.0) * xf;
        }
        let art = clone_transformed_group(d, norm_src, xf);
        let clip = clip_path_node(d.alloc_id(), shapes::rectangle(qr));
        out.push(clip_group_node(d.alloc_id(), vec![clip, art]));
    }
    out
}

fn build_kaleidoscope_tile_art(d: &mut Document, norm_src: &[Arc<Node>], tile_rect: Rect, segments: u32) -> Vec<Arc<Node>> {
    let center = tile_rect.center();
    let radius = tile_rect.width().hypot(tile_rect.height()) * 0.55;
    let wedges = segments.saturating_mul(2).max(4);
    let wedge_angle = std::f64::consts::TAU / f64::from(wedges);

    let mut sectors = Vec::with_capacity(wedges as usize);
    for k in 0..wedges {
        let a0 = f64::from(k) * wedge_angle;
        let a1 = a0 + wedge_angle;
        // Dihedral D_n symmetry: even sectors rotate by `k * wedge_angle`, odd sectors mirror across
        // the `a0 = 0` axis first so adjacent wedge boundaries match seamlessly!
        let sym_xf = if k % 2 == 0 {
            Affine::translate(center.to_vec2()) * Affine::rotate(a0) * Affine::translate(-center.to_vec2())
        } else {
            let rot = Affine::translate(center.to_vec2()) * Affine::rotate(a1) * Affine::translate(-center.to_vec2());
            rot * reflect_about(center, 0.0)
        };
        let art = clone_transformed_group(d, norm_src, sym_xf);
        let wedge_clip = clip_path_node(d.alloc_id(), wedge_polygon(center, radius, a0 - 1e-4, a1 + 1e-4));
        sectors.push(clip_group_node(d.alloc_id(), vec![wedge_clip, art]));
    }

    let outer_clip = clip_path_node(d.alloc_id(), shapes::rectangle(tile_rect));
    let mut group_children = Vec::with_capacity(sectors.len() + 1);
    group_children.push(outer_clip);
    group_children.extend(sectors);
    vec![clip_group_node(d.alloc_id(), group_children)]
}

fn build_half_drop_tile_art(d: &mut Document, norm_src: &[Arc<Node>], tile_rect: Rect) -> Vec<Arc<Node>> {
    // Center primary motif inside `tile_rect` and add 4 half-drop corner-wrap companions clipped
    // to `tile_rect` so motifs flow seamlessly across the BrickByColumn(0.5) lattice.
    let w = tile_rect.width();
    let h = tile_rect.height();
    let shrink = Affine::translate(tile_rect.center().to_vec2()) * Affine::scale(0.68) * Affine::translate(-tile_rect.center().to_vec2());
    let offsets = [Vec2::ZERO, Vec2::new(w, h * 0.5), Vec2::new(-w, -h * 0.5), Vec2::new(w, -h * 0.5), Vec2::new(-w, h * 0.5)];
    let mut children = Vec::with_capacity(offsets.len() + 1);
    children.push(clip_path_node(d.alloc_id(), shapes::rectangle(tile_rect)));
    for off in offsets {
        children.push(clone_transformed_group(d, norm_src, Affine::translate(off) * shrink));
    }
    vec![clip_group_node(d.alloc_id(), children)]
}

fn next_capture_pattern_name(d: &Document, base: &str) -> String {
    if d.pattern(base).is_none() && !d.swatch_name_taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base} {i}")).find(|n| d.pattern(n).is_none() && !d.swatch_name_taken(n)).unwrap_or_else(|| base.to_string())
}

fn run_extract_pattern_tile(s: &mut Session, p: &Value) -> Result<Value> {
    let mode_str = text_param(p, "mode", CMD_PATTERN_TILE)?.unwrap_or("kaleidoscope");
    let mode = CapturePatternMode::parse(mode_str)
        .ok_or_else(|| bad(CMD_PATTERN_TILE, format!("unknown mode `{mode_str}` (kaleidoscope|mirror|halfDrop)")))?;
    let seg_f = number_or(p, "segments", 6.0, CMD_PATTERN_TILE)?;
    if !seg_f.is_finite() || seg_f.fract() != 0.0 || !matches!(seg_f as i64, 4 | 6 | 8 | 12) {
        return Err(bad(CMD_PATTERN_TILE, "`segments` must be 4, 6, 8, or 12"));
    }
    let segments = seg_f as u32;
    let place_preview = switch_or(p, "placePreview", false, CMD_PATTERN_TILE)?;
    let wanted_name = text_param(p, "name", CMD_PATTERN_TILE)?.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);

    let roots = match given_objects(s, p, CMD_PATTERN_TILE)? {
        Some(ids) => outermost(&s.doc()?.doc, ids),
        None => selected_roots(s)?,
    };
    if roots.is_empty() {
        return Err(bad(CMD_PATTERN_TILE, "select artwork to extract a pattern tile from"));
    }

    let src_bounds = s
        .doc()?
        .doc
        .bounds_of(&roots, true)
        .filter(|b| b.width() > 1e-3 && b.height() > 1e-3)
        .ok_or_else(|| bad(CMD_PATTERN_TILE, "selected artwork has empty bounds"))?;

    let default_side = src_bounds.width().max(src_bounds.height()).clamp(24.0, 2048.0);
    let tile_size = number_or(p, "tileSize", default_side, CMD_PATTERN_TILE)?;
    if !tile_size.is_finite() || !(8.0..=4096.0).contains(&tile_size) {
        return Err(bad(CMD_PATTERN_TILE, "`tileSize` must be between 8 and 4096 pt"));
    }

    let tile_rect = Rect::new(0.0, 0.0, tile_size, tile_size);
    // Uniform-cover transform mapping `src_bounds` onto `tile_rect`.
    let scale = (tile_size / src_bounds.width()).max(tile_size / src_bounds.height());
    let to_tile = Affine::translate(tile_rect.center().to_vec2()) * Affine::scale(scale) * Affine::translate(-src_bounds.center().to_vec2());

    let (pat_name, tile_type, preview_id) = s.edit("Extract Pattern Tile", |d, sel| {
        let default_base = match mode {
            CapturePatternMode::Kaleidoscope => "Capture Kaleidoscope",
            CapturePatternMode::Mirror => "Capture Mirror",
            CapturePatternMode::HalfDrop => "Capture Half-Drop",
        };
        let name = next_capture_pattern_name(d, wanted_name.as_deref().unwrap_or(default_base));

        let raw_nodes: Vec<Arc<Node>> = roots.iter().filter_map(|id| d.node_arc(*id).cloned()).collect();
        let norm_src: Vec<Arc<Node>> = raw_nodes
            .iter()
            .map(|n| {
                let mut c = d.reid(n);
                c.transform(to_tile, true);
                Arc::new(c)
            })
            .collect();
        if norm_src.is_empty() {
            return Err(EngineError::Other("selected objects not found".into()));
        }

        let (art, tt) = match mode {
            CapturePatternMode::Kaleidoscope => (build_kaleidoscope_tile_art(d, &norm_src, tile_rect, segments), TileType::Grid),
            CapturePatternMode::Mirror => (build_mirror_tile_art(d, &norm_src, tile_rect), TileType::Grid),
            CapturePatternMode::HalfDrop => (build_half_drop_tile_art(d, &norm_src, tile_rect), TileType::BrickByColumn { offset: 0.5 }),
        };

        let mut def = PatternDef::new(&name, art);
        def.tile = tile_rect;
        def.tile_type = tt;
        add_pattern(d, def);

        let mut prev_id = None;
        if place_preview {
            let pid = d.alloc_id();
            let preview_rect = Rect::from_origin_size(Point::new(src_bounds.x1 + 24.0, src_bounds.y0), (tile_size * 2.0, tile_size * 2.0));
            let rect_node = Node::path(pid, shapes::rectangle(preview_rect), Appearance::basic(pattern_paint(&name), Paint::None, 0.0));
            let target_layer = d.default_layer();
            d.insert(target_layer, usize::MAX, rect_node)?;
            sel.set([pid]);
            prev_id = Some(pid);
        }

        Ok((name, tt, prev_id))
    })?;

    let mut out = json!({
        "name": pat_name,
        "mode": mode.id(),
        "tileType": tile_type.id(),
        "width": tile_size,
        "height": tile_size,
    });
    if let Some(pid) = preview_id {
        out["previewId"] = json!(pid.0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;
    use vectorcraft_doc::{ImageBlob, ImageObject, Node, NodeKind};
    use vectorcraft_geom::Affine;

    use crate::Session;

    fn insert_test_raster(s: &mut Session) -> u64 {
        let mut raw = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16u8 {
            for x in 0..16u8 {
                let px = match (x < 8, y < 8) {
                    (true, true) => [235, 64, 52, 255],
                    (false, true) => [36, 168, 224, 255],
                    (true, false) => [245, 198, 42, 255],
                    (false, false) => [48, 186, 112, 255],
                };
                raw.extend_from_slice(&px);
            }
        }
        let img = image::RgbaImage::from_raw(16, 16, raw).unwrap();
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let blob = ImageBlob::png(png);
        let key = blob.content_key();

        let id = s
            .edit("Insert Raster", |d, sel| {
                d.images.insert(key.clone(), blob);
                let nid = d.alloc_id();
                let mut node = Node::new(
                    nid,
                    NodeKind::Image(Box::new(ImageObject {
                        key,
                        width: 16,
                        height: 16,
                        xf: Affine::scale(6.0),
                        link: None,
                        placement: Default::default(),
                        pixels: None,
                    })),
                );
                node.name = Some("Capture Source".into());
                let layer = d.default_layer();
                d.insert(layer, usize::MAX, node)?;
                sel.set([nid]);
                Ok(nid)
            })
            .unwrap();
        id.0
    }

    #[test]
    fn extract_color_theme_from_raster_adds_five_oklab_swatches_across_moods() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        let img_id = insert_test_raster(&mut s);

        for mood in ["colorful", "bright", "muted", "deep", "dark"] {
            let res = s.execute("capture.extractColorTheme", &json!({"ids": [img_id], "mood": mood, "global": true})).unwrap();
            assert_eq!(res["mood"], mood);
            let colors = res["colors"].as_array().unwrap();
            let swatches = res["swatches"].as_array().unwrap();
            assert_eq!(colors.len(), 5);
            assert_eq!(swatches.len(), 5);

            let group_name = res["group"].as_str().unwrap();
            let grp = s.active().unwrap().doc.swatch_groups.iter().find(|g| g.name == group_name).expect("swatch group added");
            assert_eq!(grp.swatches.len(), 5);
            assert!(grp.swatches.iter().all(|w| w.global));
        }

        // Undo removes the last extracted swatch group cleanly!
        let count_before = s.active().unwrap().doc.swatch_groups.len();
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.swatch_groups.len(), count_before - 1);

        // Hostile parameter validation
        assert!(s.execute("capture.extractColorTheme", &json!({"mood": "unknownMood"})).is_err());
        assert!(s.execute("capture.extractColorTheme", &json!({"mood": 99})).is_err());
    }

    #[test]
    fn extract_pattern_tile_generates_kaleidoscope_mirror_and_half_drop_patterns() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
        let r1 = s.execute("shape.star", &json!({"cx": 100, "cy": 100, "radius1": 50, "radius2": 22, "points": 5})).unwrap()["id"].as_u64().unwrap();
        let img_id = insert_test_raster(&mut s);

        for (mode, expected_tt) in [("kaleidoscope", "grid"), ("mirror", "grid"), ("halfDrop", "brickByColumn")] {
            let res = s
                .execute(
                    "capture.extractPatternTile",
                    &json!({
                        "ids": [r1, img_id],
                        "mode": mode,
                        "segments": 6,
                        "tileSize": 120,
                        "placePreview": true
                    }),
                )
                .unwrap();
            assert_eq!(res["mode"], mode);
            assert_eq!(res["tileType"], expected_tt);
            let pat_name = res["name"].as_str().unwrap();
            let doc = &s.active().unwrap().doc;
            assert!(doc.pattern(pat_name).is_some(), "pattern `{pat_name}` must exist");
            assert!(doc.swatch(pat_name).is_some(), "pattern swatch `{pat_name}` must exist");
            assert!(res["previewId"].as_u64().is_some());
        }

        assert!(s.execute("capture.extractPatternTile", &json!({"ids": [r1], "mode": "bad"})).is_err());
        assert!(s.execute("capture.extractPatternTile", &json!({"ids": [r1], "segments": 5})).is_err());
    }
}
