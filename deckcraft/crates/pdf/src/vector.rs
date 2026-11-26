//! Vector PDF slide artwork renderer (`PdfOptions::vector = true`).
//!
//! Draws slide backgrounds, master/layout graphics, shapes, connectors, arrowheads, tables, ink
//! strokes, pictures, and text directly onto Krilla's vector PDF `Surface` instead of rasterizing
//! the whole slide to bitmaps. Complex 3-D/chart/media objects fall back to localized per-object
//! images so nothing on a slide is lost.

use craft_pdf::write::backend as krilla;
use deckcraft_geom::preset::FillMode;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::style::{
    Compound, Effects, Fill as ModelFill, Gradient, GradientFlip, GradientShape, Line, LineCap as ModelCap, LineEnd, LineJoin as ModelJoin,
    PictureAdjust, PictureFill, PictureMode,
};
use deckcraft_model::table::Table;
use deckcraft_model::{InkStroke, MediaId, Presentation, Rgba, Shape, ShapeKind, Slide, ink_pressure_is_legacy, ink_sample_width};
use deckcraft_render::{
    RenderOpts, camera_turn, crop_dest, decode_image, place_slide_with, placed_text_decorations, placed_text_outlines, render_shape, shape_geometry,
    table_cell_look, table_cell_rects, table_row_heights,
};
use krilla::Data;
use krilla::geom::{Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::paint::{
    Fill, FillRule, LineCap, LineJoin, LinearGradient, Paint, RadialGradient, SpreadMethod, Stop, Stroke, StrokeDash,
};
use krilla::surface::Surface;
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape as _, Vec2};

use crate::{Exporter, PageLayout, Result, raster_scale, straight, tf, to_path};

fn norm(v: f64) -> NormalizedF32 {
    let f = if v.is_finite() { (v as f32).clamp(0.0, 1.0) } else { 1.0 };
    NormalizedF32::new(f).unwrap_or(NormalizedF32::ONE)
}

fn rgb_color(c: Rgba) -> krilla::color::rgb::Color {
    krilla::color::rgb::Color::new(c.r, c.g, c.b)
}

fn solid_fill(c: Rgba, extra_alpha: f64, rule: FillRule) -> Option<Fill> {
    let op = (f64::from(c.a) / 255.0) * extra_alpha.clamp(0.0, 1.0);
    if op <= 1e-4 {
        return None;
    }
    Some(Fill { paint: rgb_color(c).into(), opacity: norm(op), rule })
}

fn shade_color(c: Rgba, mode: FillMode) -> Rgba {
    match mode {
        FillMode::Lighten => c.lerp(Rgba::WHITE.with_alpha(c.a), 0.4),
        FillMode::LightenLess => c.lerp(Rgba::WHITE.with_alpha(c.a), 0.2),
        FillMode::Darken => c.lerp(Rgba::BLACK.with_alpha(c.a), 0.4),
        FillMode::DarkenLess => c.lerp(Rgba::BLACK.with_alpha(c.a), 0.2),
        _ => c,
    }
}

fn spread_of(flip: Option<GradientFlip>) -> SpreadMethod {
    match flip.unwrap_or(GradientFlip::None) {
        GradientFlip::None => SpreadMethod::Pad,
        GradientFlip::X | GradientFlip::Y | GradientFlip::Xy => SpreadMethod::Reflect,
    }
}

fn gradient_paint(rctx: &Ctx<'_>, g: &Gradient, ph: Option<Rgba>, bounds: Rect, mode: FillMode, extra_alpha: f64) -> Option<(Paint, NormalizedF32)> {
    if g.stops.is_empty() || bounds.width() <= 1e-6 || bounds.height() <= 1e-6 {
        return None;
    }
    let mut stops: Vec<(f64, Rgba)> = g
        .stops
        .iter()
        .map(|st| {
            let pos = if st.pos.is_finite() { st.pos.clamp(0.0, 1.0) } else { 0.0 };
            (pos, shade_color(rctx.color(&st.color, ph), mode))
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let k_stops: Vec<Stop> = stops
        .iter()
        .map(|&(pos, c)| Stop {
            offset: norm(pos),
            color: rgb_color(c).into(),
            opacity: norm((f64::from(c.a) / 255.0) * extra_alpha),
        })
        .collect();
    let [tl, tt, tr, tb] = g.tile.map(|v| if v.is_finite() { v } else { 0.0 });
    let gx0 = bounds.x0 + tl * bounds.width();
    let gy0 = bounds.y0 + tt * bounds.height();
    let gx1 = bounds.x1 - tr * bounds.width();
    let gy1 = bounds.y1 - tb * bounds.height();
    let gw = (gx1 - gx0).max(1e-3);
    let gh = (gy1 - gy0).max(1e-3);
    let cx = (gx0 + gx1) * 0.5;
    let cy = (gy0 + gy1) * 0.5;
    let spread_method = spread_of(g.flip);
    let paint: Paint = match &g.shape {
        GradientShape::Linear { angle, .. } => {
            let rad = (if angle.is_finite() { *angle } else { 0.0 }).to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            let half = 0.5 * (dx.abs() * gw + dy.abs() * gh).max(1e-3);
            LinearGradient {
                x1: (cx - dx * half) as f32,
                y1: (cy - dy * half) as f32,
                x2: (cx + dx * half) as f32,
                y2: (cy + dy * half) as f32,
                transform: Transform::identity(),
                spread_method,
                stops: k_stops,
                anti_alias: true,
            }
            .into()
        }
        GradientShape::Path { focus, .. } => {
            let [fl, ft, fr, fb] = focus.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.5 });
            let fx = gx0 + ((fl + (1.0 - fr)) * 0.5).clamp(0.0, 1.0) * gw;
            let fy = gy0 + ((ft + (1.0 - fb)) * 0.5).clamp(0.0, 1.0) * gh;
            let r = 0.5 * gw.hypot(gh).max(1e-3);
            RadialGradient {
                fx: fx as f32,
                fy: fy as f32,
                fr: 0.0,
                cx: cx as f32,
                cy: cy as f32,
                cr: r as f32,
                transform: Transform::identity(),
                spread_method,
                stops: k_stops,
                anti_alias: true,
            }
            .into()
        }
    };
    Some((paint, NormalizedF32::ONE))
}

fn is_plain_adjust(a: &PictureAdjust) -> bool {
    a.brightness.abs() <= 1e-4
        && a.contrast.abs() <= 1e-4
        && a.saturation.unwrap_or(1.0 - 0.0).sub_one_abs() <= 1e-4
        && !a.grayscale
        && a.duotone.is_none()
        && a.clear_color.is_none()
        && a.sharpen.abs() <= 1e-4
        && a.bilevel.is_none()
        && !a.alpha_inv
}

trait SubOneAbs {
    fn sub_one_abs(self) -> f64;
}
impl SubOneAbs for f64 {
    fn sub_one_abs(self) -> f64 {
        if self.is_finite() { (self - 1.0).abs() } else { 1.0 }
    }
}

fn media_image(pres: &Presentation, id: MediaId, adj: &PictureAdjust) -> Option<Image> {
    let item = pres.media.iter().find(|m| m.id == id)?;
    if is_plain_adjust(adj) && item.data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        if let Ok(img) = Image::from_jpeg(Data::from(item.data.as_ref().clone()), true) {
            return Some(img);
        }
    }
    let rgba = decode_image(&item.data)?;
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    Some(Image::from_rgba8(rgba.into_raw(), w, h))
}

fn draw_picture_fill(
    s: &mut Surface<'_>,
    pres: &Presentation,
    pf: &PictureFill,
    clip: &BezPath,
    clip_rule: FillRule,
    bounds: Rect,
    m: Affine,
    extra_alpha: f64,
) -> bool {
    let Some(img) = media_image(pres, pf.media, &pf.adjust) else { return false };
    let Some(clip_path) = to_path(&map_path(clip, m)) else { return false };
    let target = match &pf.mode {
        PictureMode::Stretch { fill_rect } => {
            let [l, t, r, b] = fill_rect.map(|v| if v.is_finite() { v.clamp(-2.0, 2.0) } else { 0.0 });
            let inset = Rect::new(
                bounds.x0 + l * bounds.width(),
                bounds.y0 + t * bounds.height(),
                bounds.x1 - r * bounds.width(),
                bounds.y1 - b * bounds.height(),
            );
            crop_dest(inset, pf.crop)
        }
        PictureMode::Tile { .. } => crop_dest(bounds, pf.crop),
    };
    if !(target.width() > 1e-3 && target.height() > 1e-3 && target.is_finite()) {
        return false;
    }
    let Some(size) = Size::from_wh(target.width() as f32, target.height() as f32) else { return false };
    let op = pf.alpha.unwrap_or(1.0).clamp(0.0, 1.0) * extra_alpha.clamp(0.0, 1.0);
    if op <= 1e-4 {
        return true;
    }
    s.push_clip_path(&clip_path, &clip_rule);
    let pushed_op = op < 0.999;
    if pushed_op {
        s.push_opacity(norm(op));
    }
    s.push_transform(&tf(m * Affine::translate((target.x0, target.y0))));
    s.draw_image(img, size);
    s.pop();
    if pushed_op {
        s.pop();
    }
    s.pop();
    true
}

fn map_path(bp: &BezPath, m: Affine) -> BezPath {
    let mut out = BezPath::new();
    out.extend(bp.elements().iter().map(|e| m * *e));
    out
}

fn fill_subpath(
    s: &mut Surface<'_>,
    pres: &Presentation,
    rctx: &Ctx<'_>,
    fill: &ModelFill,
    ph: Option<Rgba>,
    bg_fill: &(ModelFill, Option<Rgba>),
    group_fill: Option<&(ModelFill, Option<Rgba>)>,
    subpath: &BezPath,
    even_odd: bool,
    mode: FillMode,
    bounds: Rect,
    m: Affine,
    extra_alpha: f64,
) {
    if mode == FillMode::None || subpath.elements().is_empty() {
        return;
    }
    let rule = if even_odd { FillRule::EvenOdd } else { FillRule::NonZero };
    match fill {
        ModelFill::None => {}
        ModelFill::Background => {
            fill_subpath(s, pres, rctx, &bg_fill.0, bg_fill.1, bg_fill, group_fill, subpath, even_odd, mode, bounds, m, extra_alpha);
        }
        ModelFill::Group => {
            if let Some((gf, gph)) = group_fill {
                fill_subpath(s, pres, rctx, gf, *gph, bg_fill, None, subpath, even_odd, mode, bounds, m, extra_alpha);
            }
        }
        ModelFill::Solid { color } => {
            let c = shade_color(rctx.color(color, ph), mode);
            if let (Some(f), Some(kp)) = (solid_fill(c, extra_alpha, rule), to_path(&map_path(subpath, m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
        ModelFill::Gradient(g) => {
            if let Some((paint, opacity)) = gradient_paint(rctx, g, ph, bounds, mode, extra_alpha)
                && let Some(kp) = to_path(subpath)
            {
                s.push_transform(&tf(m));
                s.set_fill(Some(Fill { paint, opacity, rule }));
                s.draw_path(&kp);
                s.set_fill(None);
                s.pop();
            }
        }
        ModelFill::Picture(pf) => {
            let _ = draw_picture_fill(s, pres, pf, subpath, rule, bounds, m, extra_alpha);
        }
        ModelFill::Pattern(pat) => {
            let fg = rctx.color(&pat.fg, ph);
            let bg = rctx.color(&pat.bg, ph);
            let tile = deckcraft_render::pattern_tile(&pat.preset);
            let mut hits = 0u32;
            for y in 0..8 {
                for x in 0..8 {
                    if deckcraft_render::pattern_is_fg(&tile, x, y) {
                        hits += 1;
                    }
                }
            }
            let ratio = f64::from(hits) / 64.0;
            let mixed = shade_color(bg.lerp(fg, ratio), mode);
            if let (Some(f), Some(kp)) = (solid_fill(mixed, extra_alpha, rule), to_path(&map_path(subpath, m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
    }
}

fn line_color_and_paint(rctx: &Ctx<'_>, line: &Line, ph: Option<Rgba>, bounds: Rect, extra_alpha: f64) -> Option<(Paint, NormalizedF32, Rgba)> {
    match &line.fill {
        None | Some(ModelFill::None) => None,
        Some(ModelFill::Solid { color }) => {
            let c = rctx.color(color, ph);
            let op = (f64::from(c.a) / 255.0) * extra_alpha.clamp(0.0, 1.0);
            (op > 1e-4).then(|| (rgb_color(c).into(), norm(op), c))
        }
        Some(ModelFill::Gradient(g)) => {
            let first = g.stops.first().map(|st| rctx.color(&st.color, ph)).unwrap_or(Rgba::BLACK);
            let (paint, op) = gradient_paint(rctx, g, ph, bounds, FillMode::Norm, extra_alpha)?;
            Some((paint, op, first))
        }
        _ => None,
    }
}

fn head_dims(end: &LineEnd, lw: f64) -> (f64, f64) {
    let wf = match end.w.as_str() {
        "sm" => 2.0,
        "lg" => 5.0,
        _ => 3.2,
    };
    let lf = match end.len.as_str() {
        "sm" => 2.2,
        "lg" => 5.5,
        _ => 3.6,
    };
    let base = lw.max(1.0);
    (base * wf, base * lf)
}

fn path_endpoints(bp: &BezPath) -> Option<((Point, Vec2), (Point, Vec2))> {
    let mut first_pt: Option<Point> = None;
    let mut first_dir: Option<Vec2> = None;
    let mut cur = Point::ZERO;
    let mut sub_start = Point::ZERO;
    let mut last_seg: Option<(Point, Point)> = None;
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                cur = p;
                sub_start = p;
            }
            PathEl::LineTo(p) => {
                let d = p - cur;
                if d.hypot() > 1e-6 {
                    if first_dir.is_none() {
                        first_pt = Some(cur);
                        first_dir = Some(d.normalize());
                    }
                    last_seg = Some((cur, p));
                }
                cur = p;
            }
            PathEl::QuadTo(c1, p) => {
                let d0 = if (c1 - cur).hypot() > 1e-6 { c1 - cur } else { p - cur };
                let d1 = if (p - c1).hypot() > 1e-6 { p - c1 } else { p - cur };
                if d0.hypot() > 1e-6 && first_dir.is_none() {
                    first_pt = Some(cur);
                    first_dir = Some(d0.normalize());
                }
                if d1.hypot() > 1e-6 {
                    last_seg = Some((p - d1, p));
                }
                cur = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                let d0 = if (c1 - cur).hypot() > 1e-6 {
                    c1 - cur
                } else if (c2 - cur).hypot() > 1e-6 {
                    c2 - cur
                } else {
                    p - cur
                };
                let d1 = if (p - c2).hypot() > 1e-6 {
                    p - c2
                } else if (p - c1).hypot() > 1e-6 {
                    p - c1
                } else {
                    p - cur
                };
                if d0.hypot() > 1e-6 && first_dir.is_none() {
                    first_pt = Some(cur);
                    first_dir = Some(d0.normalize());
                }
                if d1.hypot() > 1e-6 {
                    last_seg = Some((p - d1, p));
                }
                cur = p;
            }
            PathEl::ClosePath => {
                let d = sub_start - cur;
                if d.hypot() > 1e-6 {
                    last_seg = Some((cur, sub_start));
                }
                cur = sub_start;
            }
        }
    }
    let (fp, fd) = (first_pt?, first_dir?);
    let (la, lb) = last_seg?;
    let ld = (lb - la).normalize();
    // Outward tangent at start points backwards (`-fd`), outward tangent at end points forwards (`ld`).
    Some(((fp, -fd), (lb, ld)))
}

fn draw_arrow_end(s: &mut Surface<'_>, tip: Point, dir: Vec2, end: &LineEnd, lw: f64, color: Rgba, m: Affine, extra_alpha: f64) {
    if end.kind.is_empty() || end.kind == "none" {
        return;
    }
    let (hw, hl) = head_dims(end, lw);
    let n = Vec2::new(-dir.y, dir.x);
    let base = tip - dir * hl;
    match end.kind.as_str() {
        "arrow" => {
            let mut p = BezPath::new();
            p.move_to(base + n * (hw * 0.5));
            p.line_to(tip);
            p.line_to(base - n * (hw * 0.5));
            if let Some(kp) = to_path(&p) {
                let op = (f64::from(color.a) / 255.0) * extra_alpha;
                s.push_transform(&tf(m));
                s.set_stroke(Some(Stroke {
                    paint: rgb_color(color).into(),
                    width: lw.max(0.75) as f32,
                    miter_limit: 8.0,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    opacity: norm(op),
                    dash: None,
                }));
                s.draw_path(&kp);
                s.set_stroke(None);
                s.pop();
            }
        }
        "oval" => {
            let c = tip - dir * (hl * 0.25);
            let ell = kurbo::Ellipse::new(c, (hw * 0.5, hl * 0.5), dir.y.atan2(dir.x));
            if let (Some(f), Some(kp)) = (solid_fill(color, extra_alpha, FillRule::NonZero), to_path(&map_path(&ell.to_path(0.05), m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
        "diamond" => {
            let mid = tip - dir * (hl * 0.5);
            let back = tip - dir * hl;
            let mut p = BezPath::new();
            p.move_to(tip);
            p.line_to(mid + n * (hw * 0.5));
            p.line_to(back);
            p.line_to(mid - n * (hw * 0.5));
            p.close_path();
            if let (Some(f), Some(kp)) = (solid_fill(color, extra_alpha, FillRule::NonZero), to_path(&map_path(&p, m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
        "stealth" => {
            let notch = tip - dir * (hl * 0.68);
            let mut p = BezPath::new();
            p.move_to(tip);
            p.line_to(base + n * (hw * 0.5));
            p.line_to(notch);
            p.line_to(base - n * (hw * 0.5));
            p.close_path();
            if let (Some(f), Some(kp)) = (solid_fill(color, extra_alpha, FillRule::NonZero), to_path(&map_path(&p, m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
        _ => {
            // `triangle` and fallback
            let mut p = BezPath::new();
            p.move_to(tip);
            p.line_to(base + n * (hw * 0.5));
            p.line_to(base - n * (hw * 0.5));
            p.close_path();
            if let (Some(f), Some(kp)) = (solid_fill(color, extra_alpha, FillRule::NonZero), to_path(&map_path(&p, m))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
    }
}

fn stroke_subpath(
    s: &mut Surface<'_>,
    rctx: &Ctx<'_>,
    line: &Line,
    ph: Option<Rgba>,
    subpath: &BezPath,
    bounds: Rect,
    m: Affine,
    with_ends: bool,
    extra_alpha: f64,
) {
    if subpath.elements().is_empty() {
        return;
    }
    let lw = line.width.unwrap_or(0.75);
    if !(lw.is_finite() && lw > 0.0) {
        return;
    }
    let Some((paint, opacity, solid_c)) = line_color_and_paint(rctx, line, ph, bounds, extra_alpha) else {
        return;
    };
    let line_cap = match line.cap.unwrap_or_default() {
        ModelCap::Flat => LineCap::Butt,
        ModelCap::Round => LineCap::Round,
        ModelCap::Square => LineCap::Square,
    };
    let line_join = match line.join.unwrap_or_default() {
        ModelJoin::Round => LineJoin::Round,
        ModelJoin::Bevel => LineJoin::Bevel,
        ModelJoin::Miter => LineJoin::Miter,
    };
    let dash = line.dash.as_ref().and_then(|d| {
        let array: Vec<f32> = d.pattern().into_iter().map(|v| (v * lw.max(0.75)) as f32).filter(|v| *v > 0.0).collect();
        (!array.is_empty()).then_some(StrokeDash { array, offset: 0.0 })
    });
    let width = match line.compound.unwrap_or(Compound::Single) {
        Compound::Single => lw as f32,
        Compound::Double | Compound::ThickThin | Compound::ThinThick | Compound::Triple => (lw * 0.85) as f32,
    };
    if let Some(kp) = to_path(subpath) {
        s.push_transform(&tf(m));
        s.set_stroke(Some(Stroke {
            paint,
            width: width.max(0.25),
            miter_limit: 8.0,
            line_cap,
            line_join,
            opacity,
            dash,
        }));
        s.draw_path(&kp);
        s.set_stroke(None);
        s.pop();
    }
    if with_ends
        && (line.head.is_some() || line.tail.is_some())
        && let Some(((p0, d0), (p1, d1))) = path_endpoints(subpath)
    {
        let is_closed = subpath.elements().iter().any(|e| matches!(e, PathEl::ClosePath));
        if !is_closed {
            if let Some(head) = &line.head {
                draw_arrow_end(s, p0, d0, head, lw, solid_c, m, extra_alpha);
            }
            if let Some(tail) = &line.tail {
                draw_arrow_end(s, p1, d1, tail, lw, solid_c, m, extra_alpha);
            }
        }
    }
}

fn needs_localized_raster(s: &Shape, effects: Option<&Effects>) -> bool {
    if matches!(s.kind, ShapeKind::Chart(_) | ShapeKind::Model3d { .. } | ShapeKind::Media(_) | ShapeKind::Opaque { .. }) {
        return true;
    }
    if let ShapeKind::Picture { fill } = &s.kind
        && !is_plain_adjust(&fill.adjust)
    {
        return true;
    }
    let Some(e) = effects else { return false };
    e.soft_edge.is_some_and(|r| r > 0.0)
        || e.inner_shadow.is_some()
        || e.glow.as_ref().is_some_and(|g| g.radius > 0.0)
        || e.reflection.is_some()
        || (e.bevel.is_some() && e.raw3d.is_some())
}

fn draw_localized_shape(s: &mut Surface<'_>, pres: &Presentation, slide: &Slide, rctx: &Ctx<'_>, sh: &Shape, view: Affine, dpi: f64) {
    let x = resolve::xfrm(rctx, sh);
    let b = x.bounds().inflate(8.0, 8.0);
    if !(b.width() > 0.0 && b.height() > 0.0 && b.is_finite()) {
        return;
    }
    let scale = (dpi.clamp(72.0, 300.0) / 72.0).clamp(1.0, 4.0);
    let Some(mut img) = render_shape(pres, slide, sh.id, scale) else { return };
    if img.width == 0 || img.height == 0 {
        return;
    }
    straight(&mut img.pixels);
    let Some(size) = Size::from_wh(b.width() as f32, b.height() as f32) else { return };
    s.push_transform(&tf(view * Affine::translate((b.x0, b.y0))));
    s.draw_image(Image::from_rgba8(img.pixels, img.width, img.height), size);
    s.pop();
}

fn draw_table_vector(
    s: &mut Surface<'_>,
    pres: &Presentation,
    rctx: &Ctx<'_>,
    t: &Table,
    bg_fill: &(ModelFill, Option<Rgba>),
    m: Affine,
) {
    let heights = table_row_heights(rctx, t, &deckcraft_text::NoFields);
    let rects = table_cell_rects(t, &heights);
    if let Some(own_bg) = &t.background
        && let Some(all) = rects.iter().map(|(_, _, r)| *r).reduce(|a, b| a.union(b))
    {
        fill_subpath(s, pres, rctx, own_bg, None, bg_fill, None, &all.to_path(0.1), false, FillMode::Norm, all, m, 1.0);
    }
    for &(r, c, rect) in &rects {
        let Some(cell) = t.cell(r, c) else { continue };
        let lk = table_cell_look(rctx, t, r, c);
        let cell_bp = rect.to_path(0.1);
        match &cell.fill {
            Some(f) => fill_subpath(s, pres, rctx, f, None, bg_fill, None, &cell_bp, false, FillMode::Norm, rect, m, 1.0),
            None => match &lk.paint {
                Some((f, ph)) => fill_subpath(s, pres, rctx, f, *ph, bg_fill, None, &cell_bp, false, FillMode::Norm, rect, m, 1.0),
                None => {
                    if let Some(fc) = lk.fill.filter(|fc| fc.a > 0)
                        && let (Some(fill), Some(kp)) = (solid_fill(fc, 1.0, FillRule::NonZero), to_path(&map_path(&cell_bp, m)))
                    {
                        s.set_fill(Some(fill));
                        s.draw_path(&kp);
                        s.set_fill(None);
                    }
                }
            },
        }
    }
    // Diagonal cell borders
    for &(r, c, rect) in &rects {
        let Some(cell) = t.cell(r, c) else { continue };
        for (line_opt, p0, p1) in [
            (&cell.diag_down, (rect.x0, rect.y0), (rect.x1, rect.y1)),
            (&cell.diag_up, (rect.x0, rect.y1), (rect.x1, rect.y0)),
        ] {
            if let Some(l) = line_opt.as_ref().filter(|l| l.fill.as_ref().is_some_and(|f| !f.is_none())) {
                let mut bp = BezPath::new();
                bp.move_to(p0);
                bp.line_to(p1);
                stroke_subpath(s, rctx, l, None, &bp, rect, m, false, 1.0);
            }
        }
    }
    // 4-side cell borders
    for &(r, c, rect) in &rects {
        let Some(cell) = t.cell(r, c) else { continue };
        let lk = table_cell_look(rctx, t, r, c);
        let edges = [
            ((rect.x0, rect.y0), (rect.x0, rect.y1)),
            ((rect.x1, rect.y0), (rect.x1, rect.y1)),
            ((rect.x0, rect.y0), (rect.x1, rect.y0)),
            ((rect.x0, rect.y1), (rect.x1, rect.y1)),
        ];
        for (i, &(p0, p1)) in edges.iter().enumerate() {
            let mut bp = BezPath::new();
            bp.move_to(p0);
            bp.line_to(p1);
            if let Some(Some(explicit)) = cell.borders.get(i) {
                if explicit.fill.as_ref().is_some_and(|f| !f.is_none()) {
                    stroke_subpath(s, rctx, explicit, None, &bp, rect, m, false, 1.0);
                }
                continue;
            }
            if let Some((col, wd)) = lk.sides.get(i).copied().flatten().or(lk.border)
                && col.a > 0
                && wd > 0.0
                && let Some(kp) = to_path(&bp)
            {
                s.push_transform(&tf(m));
                s.set_stroke(Some(Stroke {
                    paint: rgb_color(col).into(),
                    width: wd.max(0.25) as f32,
                    miter_limit: 8.0,
                    line_cap: LineCap::Butt,
                    line_join: LineJoin::Miter,
                    opacity: norm(f64::from(col.a) / 255.0),
                    dash: None,
                }));
                s.draw_path(&kp);
                s.set_stroke(None);
                s.pop();
            }
        }
    }
}

fn draw_ink_vector(s: &mut Surface<'_>, strokes: &[InkStroke], view: Affine) {
    for st in strokes {
        if st.points.is_empty() || st.color.a == 0 {
            continue;
        }
        let legacy = ink_pressure_is_legacy(&st.points);
        let avg_w = st
            .points
            .iter()
            .map(|p| ink_sample_width(st.width, p.2, legacy))
            .sum::<f64>()
            / (st.points.len().max(1) as f64);
        let op = (f64::from(st.color.a) / 255.0) * if st.highlighter { 0.5 } else { 1.0 };
        if st.points.len() == 1 {
            let (px, py, pr) = st.points[0];
            let r = (ink_sample_width(st.width, pr, legacy) * 0.5).max(0.5);
            let dot = kurbo::Circle::new((px, py), r).to_path(0.05);
            if let (Some(f), Some(kp)) = (solid_fill(st.color, if st.highlighter { 0.5 } else { 1.0 }, FillRule::NonZero), to_path(&map_path(&dot, view))) {
                s.set_fill(Some(f));
                s.draw_path(&kp);
                s.set_fill(None);
            }
            continue;
        }
        let mut bp = BezPath::new();
        for (i, &(px, py, _)) in st.points.iter().enumerate() {
            if i == 0 {
                bp.move_to((px, py));
            } else {
                bp.line_to((px, py));
            }
        }
        if let Some(kp) = to_path(&bp) {
            s.push_transform(&tf(view));
            s.set_stroke(Some(Stroke {
                paint: rgb_color(st.color).into(),
                width: avg_w.max(0.5) as f32,
                miter_limit: 4.0,
                line_cap: if st.highlighter { LineCap::Square } else { LineCap::Round },
                line_join: LineJoin::Round,
                opacity: norm(op),
                dash: None,
            }));
            s.draw_path(&kp);
            s.set_stroke(None);
            s.pop();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shape_vector(
    s: &mut Surface<'_>,
    pres: &Presentation,
    slide: &Slide,
    rctx: &Ctx<'_>,
    sh: &Shape,
    parent: Affine,
    view: Affine,
    bg_fill: &(ModelFill, Option<Rgba>),
    group_fill: Option<&(ModelFill, Option<Rgba>)>,
    dpi: f64,
    depth: usize,
) {
    if sh.hidden || depth > 64 {
        return;
    }
    let (effects, eff_ph) = resolve::effects(rctx, sh);
    if depth == 0 && needs_localized_raster(sh, effects.as_ref()) && slide.shape(sh.id).is_some() {
        draw_localized_shape(s, pres, slide, rctx, sh, view, dpi);
        return;
    }
    let x = resolve::xfrm(rctx, sh);
    if !(x.w.is_finite() && x.h.is_finite() && x.x.is_finite() && x.y.is_finite() && x.w >= 0.0 && x.h >= 0.0) {
        return;
    }
    if let ShapeKind::Group { children, child } = &sh.kind {
        let ch = Rect::new(child.x, child.y, child.x + child.w, child.y + child.h);
        let gm = parent * deckcraft_geom::group_child_affine(&x, ch);
        let own_gf = resolve::fill(rctx, sh);
        let gf_pair = own_gf.0.map(|f| (f, own_gf.1));
        let next_gf = gf_pair.as_ref().or(group_fill);
        for c in children {
            draw_shape_vector(s, pres, slide, rctx, c, gm, view, bg_fill, next_gf, dpi, depth + 1);
        }
        return;
    }
    if let ShapeKind::Ink { strokes } = &sh.kind {
        draw_ink_vector(s, strokes, view * parent);
        return;
    }

    let mut local = parent * x.affine();
    if let Some(cam) = camera_turn(rctx, sh, x.w, x.h) {
        local *= cam;
    }
    let m = view * local;
    let bounds = Rect::new(0.0, 0.0, x.w, x.h);

    if let ShapeKind::Table(t) = &sh.kind {
        draw_table_vector(s, pres, rctx, t, bg_fill, m);
        return;
    }

    let geo = shape_geometry(sh, x.w, x.h);
    let (fill, fill_ph) = resolve::fill(rctx, sh);
    let (line, line_ph) = resolve::line(rctx, sh);

    // Crisp vector drop shadow under outer shapes when configured.
    if let Some(eff) = &effects
        && let Some(shd) = &eff.outer_shadow
        && !shd.inner
    {
        let sc = rctx.color(&shd.color, eff_ph);
        let rad = shd.dir.to_radians();
        let dx = shd.dist * rad.cos();
        let dy = shd.dist * rad.sin();
        let sm = view * Affine::translate((dx, dy)) * local;
        let outline = geo.outline();
        let soft_factor: f64 = (1.0 / (1.0 + shd.blur * 0.08)).clamp(0.25, 1.0);
        if fill.as_ref().is_some_and(|f| !f.is_none()) || matches!(sh.kind, ShapeKind::Picture { .. }) {
            if let (Some(sf), Some(kp)) = (solid_fill(sc, soft_factor, FillRule::NonZero), to_path(&map_path(&outline, sm))) {
                s.set_fill(Some(sf));
                s.draw_path(&kp);
                s.set_fill(None);
            }
        }
    }

    if let ShapeKind::Picture { fill: pf } = &sh.kind {
        let outline = geo.outline();
        let _ = draw_picture_fill(s, pres, pf, &outline, FillRule::NonZero, bounds, m, 1.0);
        if line.fill.as_ref().is_some_and(|f| !f.is_none()) {
            stroke_subpath(s, rctx, &line, line_ph, &outline, bounds, m, false, 1.0);
        }
        return;
    }

    if let Some(f) = &fill
        && !f.is_none()
    {
        for sp in &geo.paths {
            fill_subpath(s, pres, rctx, f, fill_ph, bg_fill, group_fill, &sp.path, sp.even_odd, sp.fill, bounds, m, 1.0);
        }
    }

    if line.fill.as_ref().is_some_and(|f| !f.is_none()) {
        let is_conn = matches!(sh.kind, ShapeKind::Connector { .. })
            || sh.geom.preset_name().is_some_and(|n| n == "line" || n.contains("Connector") || n.starts_with("curvedConnector") || n.starts_with("bentConnector"));
        for sp in geo.paths.iter().filter(|sp| sp.stroke) {
            stroke_subpath(s, rctx, &line, line_ph, &sp.path, bounds, m, is_conn || geo.paths.len() == 1, 1.0);
        }
    }
}

fn should_draw_inherited_shape(_slide: &Slide, sh: &Shape) -> bool {
    sh.ph.is_none()
}

impl Exporter<'_> {
    /// Draw slide `i` into page-point rectangle `b` using vector paths, gradients, images, and
    /// selectable glyphs (or outlined glyph curves when `text_layer` is `false`).
    pub(crate) fn slide_vector(&mut self, s: &mut Surface<'_>, i: usize, b: Rect) -> Result<()> {
        let pres = self.pres;
        let Some(slide) = pres.slides.get(i) else { return Ok(()) };
        let Some(rctx) = Ctx::for_slide(pres, slide) else { return Ok(()) };
        let (sw, sh) = (pres.slide_size.width.max(1.0), pres.slide_size.height.max(1.0));
        let k = b.width() / sw;
        let ky = b.height() / sh;
        let view = Affine::translate((b.x0, b.y0)) * Affine::scale_non_uniform(k, ky);

        let slide_rect = Rect::new(0.0, 0.0, sw, sh);
        let slide_path = slide_rect.to_path(0.1);
        let page_clip = b.to_path(0.1);
        let Some(clip_path) = to_path(&page_clip) else { return Ok(()) };

        s.push_clip_path(&clip_path, &FillRule::NonZero);

        // 1. White slide underlay + resolved background fill
        if let (Some(white), Some(kp)) = (solid_fill(Rgba::WHITE, 1.0, FillRule::NonZero), to_path(&page_clip)) {
            s.set_fill(Some(white));
            s.draw_path(&kp);
            s.set_fill(None);
        }
        let bg = resolve::background(&rctx, Some(slide));
        fill_subpath(s, pres, &rctx, &bg.0, bg.1, &bg, None, &slide_path, false, FillMode::Norm, slide_rect, view, 1.0);

        // 2. Master, layout, and slide shapes in z-order
        let (show_layout, show_master) = resolve::show_master_shapes(slide, rctx.layout);
        if show_master {
            let mctx = Ctx::for_master(pres, rctx.master);
            for msh in &rctx.master.shapes {
                if should_draw_inherited_shape(slide, msh) {
                    draw_shape_vector(s, pres, slide, &mctx, msh, Affine::IDENTITY, view, &bg, None, self.opts.dpi, 0);
                }
            }
        }
        if show_layout && let Some(layout) = rctx.layout {
            let lctx = Ctx::for_layout(pres, rctx.master, layout);
            for lsh in &layout.shapes {
                if should_draw_inherited_shape(slide, lsh) {
                    draw_shape_vector(s, pres, slide, &lctx, lsh, Affine::IDENTITY, view, &bg, None, self.opts.dpi, 0);
                }
            }
        }
        for ssh in &slide.shapes {
            draw_shape_vector(s, pres, slide, &rctx, ssh, Affine::IDENTITY, view, &bg, None, self.opts.dpi, 0);
        }

        // 3. Laid-out slide text (master/layout non-placeholder text, slide shapes, table cells, footer fields)
        let scale = raster_scale(b.width(), (sw, sh), self.opts.dpi);
        let ropts = RenderOpts { scale, threads: 4, now: deckcraft_text::datetime::DateTime::now_local(), ..Default::default() };
        let mut text_err: Option<crate::PdfError> = None;
        for pt in place_slide_with(pres, i, &ropts).texts {
            if self.opts.text_layer {
                for (bp, col, op) in placed_text_decorations(&pt, view, Some(true)) {
                    if let (Some(f), Some(kp)) = (solid_fill(col, op, FillRule::NonZero), to_path(&bp)) {
                        s.set_fill(Some(f));
                        s.draw_path(&kp);
                        s.set_fill(None);
                    }
                }
                if let Err(err) = self.text_layer(s, &pt, view, true) {
                    text_err = Some(err);
                    break;
                }
                for (bp, col, op) in placed_text_decorations(&pt, view, Some(false)) {
                    if let (Some(f), Some(kp)) = (solid_fill(col, op, FillRule::NonZero), to_path(&bp)) {
                        s.set_fill(Some(f));
                        s.draw_path(&kp);
                        s.set_fill(None);
                    }
                }
            } else {
                for (bp, col, op) in placed_text_outlines(&pt, view) {
                    if let (Some(f), Some(kp)) = (solid_fill(col, op, FillRule::NonZero), to_path(&bp)) {
                        s.set_fill(Some(f));
                        s.draw_path(&kp);
                        s.set_fill(None);
                    }
                }
            }
        }

        s.pop(); // clip_path
        if let Some(err) = text_err {
            return Err(err);
        }

        // 4. Thin slide frame on notes/handout pages
        if self.opts.frame_slides && self.opts.layout != PageLayout::Slides {
            if let Some(path) = to_path(&page_clip) {
                s.set_fill(None);
                s.set_stroke(Some(Stroke {
                    paint: krilla::color::rgb::Color::new(0x80, 0x80, 0x80).into(),
                    width: 0.5,
                    ..Default::default()
                }));
                s.draw_path(&path);
                s.set_stroke(None);
            }
        }
        Ok(())
    }
}
