//! Procedural Text-to-Vector / Generative Shape Fill (`object.generativeShapeFill` — Illustrator
//! 2027 `31.0` `생성형 모양 채우기`): synthesizes deterministic vector motif sub-paths (`geometric`,
//! `botanical`, `halftone`, `mosaic`) guided by an optional text prompt and clips every motif cleanly
//! to the selected closed vector shapes via `vectorcraft_pathops::boolean` in one Undo step.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, rgb_to_hsb};
use vectorcraft_doc::{Appearance, AppearanceItem, FillLayer, Node, NodeId, NodeKind};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect, shapes};
use vectorcraft_pathops::{BoolOp, boolean};

use super::edit::{outermost, selected_roots};
use super::{
    CommandSpec, Result, Session, bad, cmd, given_objects, has_selection, number_or,
    switch_or, text_param,
};

const CMD: &str = "object.generativeShapeFill";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "object.generativeShapeFill",
        "Generative Shape Fill…",
        ["Object"],
        None,
        "{ids?|id?, style?: \"geometric\"|\"botanical\"|\"halftone\"|\"mosaic\", prompt?: string, seed?: int=42, density?: %=50, scale?: %=100, keepOutline?: bool=true} fill selected closed paths with procedural vector motif subpaths clipped to each shape → {ids, generated, style}",
        has_selection,
        run_generative_shape_fill
    )]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MotifStyle {
    Geometric,
    Botanical,
    Halftone,
    Mosaic,
}

impl MotifStyle {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "geometric" => Some(Self::Geometric),
            "botanical" => Some(Self::Botanical),
            "halftone" => Some(Self::Halftone),
            "mosaic" => Some(Self::Mosaic),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Geometric => "geometric",
            Self::Botanical => "botanical",
            Self::Halftone => "halftone",
            Self::Mosaic => "mosaic",
        }
    }
}

#[derive(Clone, Copy)]
struct GenParams {
    style: MotifStyle,
    seed: u64,
    density: f64,
    scale: f64,
    keep_outline: bool,
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn hsb_to_rgb(h_deg: f32, s: f32, v: f32) -> Color {
    let h = h_deg.rem_euclid(360.0) / 60.0;
    let i = h.floor() as i32;
    let f = h - h.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::rgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

fn build_palette(base: Option<Color>, prompt: &str) -> [Color; 5] {
    let lower = prompt.to_ascii_lowercase();
    let (base_h, base_s, base_b) = if lower.contains("forest") || lower.contains("leaf") || lower.contains("botanical") || lower.contains("숲") {
        (138.0, 0.58, 0.72)
    } else if lower.contains("ocean") || lower.contains("sea") || lower.contains("cyber") || lower.contains("바다") {
        (202.0, 0.68, 0.82)
    } else if lower.contains("sunset") || lower.contains("gold") || lower.contains("fire") || lower.contains("노을") {
        (24.0, 0.76, 0.90)
    } else if lower.contains("rose") || lower.contains("floral") || lower.contains("berry") || lower.contains("꽃") {
        (336.0, 0.64, 0.86)
    } else if let Some(c) = base {
        let [h, s, b] = rgb_to_hsb(c.to_rgb());
        (h, s.clamp(0.25, 0.85), b.clamp(0.35, 0.92))
    } else {
        (215.0, 0.62, 0.82)
    };
    [
        hsb_to_rgb(base_h, (base_s * 0.45).clamp(0.1, 1.0), (base_b * 1.12).clamp(0.2, 0.97)),
        hsb_to_rgb(base_h + 18.0, base_s, base_b),
        hsb_to_rgb(base_h - 22.0, (base_s * 0.85).clamp(0.15, 1.0), (base_b * 0.88).clamp(0.18, 0.95)),
        hsb_to_rgb(base_h + 155.0, (base_s * 0.75).clamp(0.2, 0.95), (base_b * 0.94).clamp(0.25, 0.96)),
        hsb_to_rgb(base_h, (base_s * 1.05).clamp(0.2, 1.0), (base_b * 0.62).clamp(0.15, 0.85)),
    ]
}

fn leaf_motif(cx: f64, cy: f64, len: f64, width: f64, angle_rad: f64) -> PathData {
    let (sa, ca) = angle_rad.sin_cos();
    let rot = |lx: f64, ly: f64| Point::new(cx + lx * ca - ly * sa, cy + lx * sa + ly * ca);
    let mut bp = BezPath::new();
    bp.move_to(rot(-len * 0.5, 0.0));
    bp.curve_to(rot(-len * 0.15, -width), rot(len * 0.25, -width * 0.85), rot(len * 0.5, 0.0));
    bp.curve_to(rot(len * 0.25, width * 0.85), rot(-len * 0.15, width), rot(-len * 0.5, 0.0));
    bp.close_path();
    PathData::from_bezpath(&bp)
}

fn rotated_poly(cx: f64, cy: f64, r: f64, sides: usize, angle_rad: f64) -> PathData {
    let mut bp = BezPath::new();
    for i in 0..sides {
        let a = angle_rad + (i as f64) * std::f64::consts::TAU / (sides as f64);
        let pt = Point::new(cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            bp.move_to(pt);
        } else {
            bp.line_to(pt);
        }
    }
    bp.close_path();
    PathData::from_bezpath(&bp)
}

fn generate_motifs(bounds: Rect, gp: GenParams, palette: &[Color; 5]) -> Vec<(PathData, Color)> {
    let mut rng = SplitMix64::new(gp.seed);
    let span = bounds.width().hypot(bounds.height()).max(24.0);
    // Grid resolution scales smoothly with density (10..100%) and inverse scale.
    let cells = ((3.0 + gp.density * 0.09) / (gp.scale / 100.0).clamp(0.35, 2.5)).round().clamp(3.0, 14.0) as usize;
    let cols = (cells as f64 * (bounds.width() / bounds.height().max(1.0)).sqrt()).round().clamp(3.0, 16.0) as usize;
    let rows = (cells as f64 * (bounds.height() / bounds.width().max(1.0)).sqrt()).round().clamp(3.0, 16.0) as usize;
    let step_x = bounds.width() / (cols as f64);
    let step_y = bounds.height() / (rows as f64);
    let unit = step_x.min(step_y).max(4.0) * (gp.scale / 100.0).clamp(0.35, 2.5);
    let mut out = Vec::with_capacity(cols * rows + 2);

    match gp.style {
        MotifStyle::Geometric => {
            for r in 0..rows {
                for c in 0..cols {
                    let cx = bounds.x0 + (c as f64 + 0.5) * step_x + rng.range(-0.18, 0.18) * step_x;
                    let cy = bounds.y0 + (r as f64 + 0.5) * step_y + rng.range(-0.18, 0.18) * step_y;
                    let rad = unit * rng.range(0.34, 0.56);
                    let kind = (rng.next_u64() % 3) as usize;
                    let shape = match kind {
                        0 => shapes::ellipse(Rect::new(cx - rad, cy - rad, cx + rad, cy + rad)),
                        1 => rotated_poly(cx, cy, rad * 1.1, 3, rng.range(0.0, std::f64::consts::TAU)),
                        _ => rotated_poly(cx, cy, rad, 4, std::f64::consts::FRAC_PI_4 + rng.range(-0.25, 0.25)),
                    };
                    let col = palette[1 + ((rng.next_u64() as usize) % 4)];
                    out.push((shape, col));
                }
            }
        }
        MotifStyle::Botanical => {
            for r in 0..rows {
                for c in 0..cols {
                    let cx = bounds.x0 + (c as f64 + 0.5) * step_x + rng.range(-0.25, 0.25) * step_x;
                    let cy = bounds.y0 + (r as f64 + 0.5) * step_y + rng.range(-0.25, 0.25) * step_y;
                    let petals = 2 + ((rng.next_u64() % 2) as usize);
                    let base_ang = rng.range(0.0, std::f64::consts::TAU);
                    for k in 0..petals {
                        let ang = base_ang + (k as f64) * 0.85;
                        let leaf = leaf_motif(cx, cy, unit * rng.range(0.75, 1.15), unit * 0.28, ang);
                        let col = palette[1 + ((r + c + k) % 4)];
                        out.push((leaf, col));
                    }
                }
            }
        }
        MotifStyle::Halftone => {
            let phase = rng.range(0.0, std::f64::consts::TAU);
            for r in 0..rows {
                for c in 0..cols {
                    let cx = bounds.x0 + (c as f64 + 0.5) * step_x;
                    let cy = bounds.y0 + (r as f64 + 0.5) * step_y;
                    let dist = Point::new(cx, cy).distance(bounds.center()) / (span * 0.5);
                    let wave = 0.5 + 0.5 * (dist * 5.0 + phase).cos();
                    let rad = (unit * (0.14 + 0.34 * wave)).max(1.2);
                    let dot = shapes::ellipse(Rect::new(cx - rad, cy - rad, cx + rad, cy + rad));
                    let idx = ((wave * 3.99) as usize).min(3) + 1;
                    out.push((dot, palette[idx]));
                }
            }
        }
        MotifStyle::Mosaic => {
            // Jittered lattice vertices shrunk inward per quad cell for crisp stained-glass / mosaic tiles.
            let mut grid = vec![vec![Point::ZERO; cols + 1]; rows + 1];
            for (r, row_pts) in grid.iter_mut().enumerate() {
                for (c, pt) in row_pts.iter_mut().enumerate() {
                    let jx = if c == 0 || c == cols { 0.0 } else { rng.range(-0.28, 0.28) * step_x };
                    let jy = if r == 0 || r == rows { 0.0 } else { rng.range(-0.28, 0.28) * step_y };
                    *pt = Point::new(bounds.x0 + (c as f64) * step_x + jx, bounds.y0 + (r as f64) * step_y + jy);
                }
            }
            for r in 0..rows {
                for c in 0..cols {
                    let quad = [grid[r][c], grid[r][c + 1], grid[r + 1][c + 1], grid[r + 1][c]];
                    let center = Point::new(
                        (quad[0].x + quad[1].x + quad[2].x + quad[3].x) * 0.25,
                        (quad[0].y + quad[1].y + quad[2].y + quad[3].y) * 0.25,
                    );
                    let mut bp = BezPath::new();
                    for (i, v) in quad.iter().enumerate() {
                        let inset_pt = Point::new(center.x + (v.x - center.x) * 0.86, center.y + (v.y - center.y) * 0.86);
                        if i == 0 {
                            bp.move_to(inset_pt);
                        } else {
                            bp.line_to(inset_pt);
                        }
                    }
                    bp.close_path();
                    let col = palette[1 + ((rng.next_u64() as usize) % 4)];
                    out.push((PathData::from_bezpath(&bp), col));
                }
            }
        }
    }
    out
}

fn extract_target_shape(node: &Node) -> Option<(PathData, FillRule, Option<Color>)> {
    let base_col = node.appearance.fill().and_then(|f| match &f.paint {
        Paint::Solid { color: c, .. } => Some(*c),
        Paint::Gradient(g) => g.gradient.stops.first().map(|s| s.color),
        _ => None,
    });
    match &node.kind {
        NodeKind::Path { path, rule, guide: false, .. } if path.subpaths.iter().any(|sp| sp.anchors.len() >= 3) => {
            Some((path.clone(), *rule, base_col))
        }
        NodeKind::Compound { children, rule } => {
            let mut merged = PathData::new(Vec::new());
            for ch in children {
                if let NodeKind::Path { path, .. } = &ch.kind {
                    merged.subpaths.extend(path.subpaths.iter().cloned());
                }
            }
            (!merged.subpaths.is_empty()).then_some((merged, *rule, base_col))
        }
        _ => None,
    }
}

fn parse_gen_params(p: &Value) -> Result<(GenParams, String)> {
    let style_str = text_param(p, "style", CMD)?.unwrap_or("geometric");
    let prompt = text_param(p, "prompt", CMD)?.unwrap_or("").to_string();
    let inferred_style = if p.get("style").is_none_or(Value::is_null) && !prompt.is_empty() {
        let low = prompt.to_ascii_lowercase();
        if low.contains("leaf") || low.contains("botanical") || low.contains("floral") || low.contains("꽃") || low.contains("잎") {
            MotifStyle::Botanical
        } else if low.contains("mosaic") || low.contains("tile") || low.contains("glass") || low.contains("모자이크") {
            MotifStyle::Mosaic
        } else if low.contains("dot") || low.contains("halftone") || low.contains("wave") || low.contains("망점") {
            MotifStyle::Halftone
        } else {
            MotifStyle::Geometric
        }
    } else {
        MotifStyle::parse(style_str).ok_or_else(|| bad(CMD, format!("unknown style `{style_str}` (geometric|botanical|halftone|mosaic)")))?
    };

    let seed_f = number_or(p, "seed", 42.0, CMD)?;
    if !seed_f.is_finite() || seed_f < 0.0 || seed_f.fract() != 0.0 {
        return Err(bad(CMD, "`seed` must be a non-negative integer"));
    }
    let density = number_or(p, "density", 50.0, CMD)?;
    if !density.is_finite() || !(5.0..=100.0).contains(&density) {
        return Err(bad(CMD, "`density` must be between 5 and 100"));
    }
    let scale = number_or(p, "scale", 100.0, CMD)?;
    if !scale.is_finite() || !(25.0..=300.0).contains(&scale) {
        return Err(bad(CMD, "`scale` must be between 25 and 300"));
    }
    let keep_outline = switch_or(p, "keepOutline", true, CMD)?;
    let combined_seed = (seed_f as u64) ^ fnv1a(&prompt);
    Ok((
        GenParams { style: inferred_style, seed: combined_seed, density, scale, keep_outline },
        prompt,
    ))
}

fn solid_fill_app(color: Color) -> Appearance {
    Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(Paint::solid(color)))], ..Appearance::default() }
}

fn run_generative_shape_fill(s: &mut Session, p: &Value) -> Result<Value> {
    let (gp, prompt) = parse_gen_params(p)?;
    let roots = match given_objects(s, p, CMD)? {
        Some(ids) => outermost(&s.doc()?.doc, ids),
        None => selected_roots(s)?,
    };

    let st = s.doc()?;
    type PreparedFill = (NodeId, Node, Vec<(PathData, Color)>, Color);
    let mut prepared: Vec<PreparedFill> = Vec::new();
    for &id in &roots {
        let Some(node) = st.doc.node(id) else { continue };
        let Some((clip_path, clip_rule, base_col)) = extract_target_shape(node) else { continue };
        let Some(bounds) = clip_path.bounds().filter(|b| b.width() > 0.5 && b.height() > 0.5) else { continue };
        let palette = build_palette(base_col, &prompt);
        let raw_motifs = generate_motifs(bounds, gp, &palette);
        let mut clipped = Vec::new();
        for (m_path, col) in raw_motifs {
            let inter = boolean(&clip_path, clip_rule, &m_path, FillRule::NonZero, BoolOp::Intersect);
            if !inter.subpaths.is_empty() {
                clipped.push((inter, col));
            }
        }
        if !clipped.is_empty() {
            prepared.push((id, node.clone(), clipped, palette[0]));
        }
    }
    if prepared.is_empty() {
        return Err(bad(CMD, "select at least one closed path or compound path with area"));
    }

    let (group_ids, total_subpaths) = s.edit("Generative Shape Fill", |d, sel| {
        let mut out_ids = Vec::with_capacity(prepared.len());
        let mut generated = 0usize;
        for (orig_id, orig_node, clipped, bg_col) in prepared {
            let mut children = Vec::with_capacity(clipped.len() + 2);
            if gp.keep_outline {
                let mut bg = orig_node.clone();
                bg.id = d.alloc_id();
                bg.appearance = solid_fill_app(bg_col).into();
                children.push(Arc::new(bg));
            }
            for (c_path, col) in clipped {
                generated += 1;
                children.push(Arc::new(Node::path(d.alloc_id(), c_path, solid_fill_app(col))));
            }
            if gp.keep_outline
                && let Some(st_layer) = orig_node.appearance.stroke().filter(|st| st.visible && st.width > 0.0).cloned()
            {
                let mut frame = orig_node;
                frame.id = d.alloc_id();
                frame.appearance = Appearance { items: vec![AppearanceItem::Stroke(st_layer)], ..Appearance::default() }.into();
                children.push(Arc::new(frame));
            }
            let gid = d.alloc_id();
            let mut g = Node::group(gid, children);
            g.name = Some("Generative Shape Fill".into());
            d.wrap(&[orig_id], |_| g)?;
            out_ids.push(gid);
        }
        sel.set(out_ids.iter().copied());
        Ok((out_ids, generated))
    })?;

    Ok(json!({
        "ids": group_ids.iter().map(|i| i.0).collect::<Vec<_>>(),
        "generated": total_subpaths,
        "style": gp.style.id(),
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::Session;

    #[test]
    fn generative_shape_fill_clips_motifs_across_styles_and_supports_undo() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();

        for style in ["geometric", "botanical", "halftone", "mosaic"] {
            let c = s.execute("shape.ellipse", &json!({"x": 40, "y": 40, "width": 160, "height": 160})).unwrap();
            let cid = c["id"].as_u64().unwrap();
            let res = s
                .execute(
                    "object.generativeShapeFill",
                    &json!({"ids": [cid], "style": style, "prompt": "sunset forest", "seed": 7, "density": 55}),
                )
                .unwrap();
            assert_eq!(res["style"], style);
            assert!(res["generated"].as_u64().unwrap() > 0);
            let gid = vectorcraft_doc::NodeId(res["ids"][0].as_u64().unwrap());
            let gb = s.active().unwrap().doc.node(gid).unwrap().geometric_bounds().unwrap();
            assert!(gb.x0 >= 39.5 && gb.y0 >= 39.5 && gb.x1 <= 200.5 && gb.y1 <= 200.5, "motifs stay clipped inside circle: {gb:?}");
            s.execute("edit.undo", &json!({})).unwrap();
            assert!(s.active().unwrap().doc.node(vectorcraft_doc::NodeId(cid)).is_some());
        }
    }
}
