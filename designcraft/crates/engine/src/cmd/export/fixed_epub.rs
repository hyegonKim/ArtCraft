//! Scene-graph builder for True HTML5 + CSS3 Fixed-Layout EPUB 3.0 export.
//!
//! Walks each page's visible, printable layers back-to-front (parent spread items composed per
//! page + document spread items), emitting positioned `<div>` frames, live selectable Unicode
//! `<span>` text runs with hyperlinks, inline `<svg>` vector paths and gradients, and `<img>`
//! placed graphics.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

use designcraft_color::swatch::resolve_gradient;
use designcraft_color::{BlendMode, GradientKind};
use designcraft_compose::{Cache, ComposedStory, FrameText};
use designcraft_doc::{
    ANCHOR_MARK, ButtonAction, Cap, Content, Document, Fill, Graphic, HyperlinkDest,
    HyperlinkSource, Item, ItemId, Join, Shape, StoryId, Stroke, StrokeType, TextFrame,
};
use designcraft_epub::{
    EmbeddedFont, FixedBox, FixedElement, FixedPageScene, FixedRaster, FixedTextSpan,
    FixedVectorSvg,
};
use designcraft_geom::corners::{self, CornerShape};
use designcraft_geom::kurbo::{PathEl, Shape as _};
use designcraft_geom::{Affine, BezPath, Point, Rect};

pub(crate) struct FixedSceneBuilder<'a> {
    doc: &'a Document,
    cache: &'a Cache,
    pub warnings: &'a mut Vec<String>,
    item_links: HashMap<ItemId, String>,
    /// Per-story `(start, end, href)` text hyperlink spans.
    text_links: HashMap<StoryId, Vec<(usize, usize, String)>>,
    used_faces: BTreeSet<(String, String)>,
    svg_grad_seq: u32,
    page_offset: usize,
    total_pages: usize,
}

impl<'a> FixedSceneBuilder<'a> {
    pub(crate) fn new(
        doc: &'a Document,
        cache: &'a Cache,
        warnings: &'a mut Vec<String>,
        page_offset: usize,
        total_pages: usize,
    ) -> Self {
        let anchor_pages = collect_anchor_pages(doc, cache);
        let mut item_links = HashMap::new();
        let mut text_links: HashMap<StoryId, Vec<(usize, usize, String)>> = HashMap::new();
        for hl in &doc.hyperlinks {
            let Some(href) = dest_href(&hl.dest, &anchor_pages, page_offset) else {
                continue;
            };
            match &hl.source {
                HyperlinkSource::Item { id } => {
                    item_links.insert(*id, href);
                }
                HyperlinkSource::Text { story, start, end } if end > start => {
                    text_links.entry(*story).or_default().push((*start, *end, href));
                }
                HyperlinkSource::Text { .. } => {}
            }
        }
        Self {
            doc,
            cache,
            warnings,
            item_links,
            text_links,
            used_faces: BTreeSet::new(),
            svg_grad_seq: 0,
            page_offset,
            total_pages: total_pages.max(doc.page_count()).max(1),
        }
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.warnings.contains(&msg) {
            self.warnings.push(msg);
        }
    }

    pub(crate) fn build_all_pages(&mut self) -> Vec<FixedPageScene> {
        let mut out = Vec::with_capacity(self.doc.page_count());
        for abs in 0..self.doc.page_count() {
            if let Some(scene) = self.build_page(abs) {
                out.push(scene);
            }
        }
        out
    }

    pub(crate) fn collect_embedded_fonts(&mut self) -> Vec<EmbeddedFont> {
        for (fam, sty) in designcraft_epub::stylesheet_fonts(self.doc) {
            self.used_faces.insert((fam, sty));
        }
        let db = designcraft_fonts::FontDb::global().scoped(self.doc.font_scope);
        let mut out = Vec::new();
        let faces: Vec<(String, String)> = self.used_faces.iter().cloned().collect();
        for (family, style) in faces {
            let face = db.face(&family, &style);
            if !face.family.eq_ignore_ascii_case(&family) {
                self.warn(format!("font `{family} {style}` is not available; not embedded"));
            } else if face.restricted_licence() || face.bitmap_only() {
                self.warn(format!("font `{family} {style}`: its licence doesn't allow embedding; not embedded"));
            } else if face.data().starts_with(b"ttcf") {
                self.warn(format!("font `{family} {style}` is part of a font collection; not embedded"));
            } else if !out.iter().any(|e: &EmbeddedFont| e.family == family && e.style == face.style) {
                out.push(EmbeddedFont {
                    family,
                    style: face.style.clone(),
                    data: face.data().to_vec(),
                });
            }
        }
        out
    }

    pub(crate) fn build_page(&mut self, abs: usize) -> Option<FixedPageScene> {
        let (si, pi) = self.doc.page_loc(abs)?;
        let sp = self.doc.spreads.get(si)?;
        let page = sp.pages.get(pi)?;
        let bounds = page.bounds();
        let to_page = Affine::translate(-bounds.origin().to_vec2());
        let b = self.doc.settings.bleed;
        let clip = Rect::new(
            bounds.x0 - b[2].max(b[3]),
            bounds.y0 - b[0],
            bounds.x1 + b[2].max(b[3]),
            bounds.y1 + b[1],
        );
        let overridden = self
            .doc
            .overridden_parent_items(designcraft_doc::SpreadRef::Doc(si), pi);

        let mut elements = Vec::new();
        for layer in self.doc.layers.iter().rev() {
            if !layer.visible || !layer.printable {
                continue;
            }
            if page.show_parent_items
                && let Some((ppi, ppage)) = self.doc.parent_page_for(abs)
                && let Some(parent) = self.doc.parents.get(ppi)
                && let Some(pp) = parent.pages.get(ppage)
            {
                let dx = page.x - pp.x;
                for it in &parent.items {
                    if it.layer != layer.id || overridden.contains(&it.id) {
                        continue;
                    }
                    if parent.pages.len() > 1
                        && parent.page_at_x(it.bounds().center().x) != Some(ppage)
                    {
                        continue;
                    }
                    self.emit_item(
                        it,
                        Affine::translate((dx, 0.0)),
                        to_page,
                        clip,
                        abs,
                        Some(abs),
                        &mut elements,
                    );
                }
            }
            for it in &sp.items {
                if it.layer == layer.id {
                    self.emit_item(it, Affine::IDENTITY, to_page, clip, abs, None, &mut elements);
                }
            }
        }

        let paper_bg = css_color(self.doc, "[Paper]", 1.0).filter(|c| c != "#ffffff");
        Some(FixedPageScene {
            label: self.doc.page_name(abs),
            width: bounds.width(),
            height: bounds.height(),
            background_css: paper_bg,
            elements,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_item(
        &mut self,
        it: &Item,
        parent: Affine,
        to_page: Affine,
        clip: Rect,
        abs_page: usize,
        parent_page: Option<usize>,
        out: &mut Vec<FixedElement>,
    ) {
        if it.hidden || it.nonprinting {
            return;
        }
        let reach = it.stroke.extent() + 4.0;
        let vb = parent.transform_rect_bbox(it.bounds()).inflate(reach, reach);
        if vb.x1 < clip.x0 || vb.x0 > clip.x1 || vb.y1 < clip.y0 || vb.y0 > clip.y1 {
            return;
        }
        let xf = parent * it.xf;
        let m = to_page * xf;

        if it.is_group() {
            let mut group_children = Vec::new();
            for child in it.shown_children() {
                self.emit_item(child, xf, to_page, clip, abs_page, parent_page, &mut group_children);
            }
            if group_children.is_empty() {
                return;
            }
            let op = it.opacity.clamp(0.0, 1.0);
            let bm = css_blend_mode(it.blend);
            let href = self.item_href(it, abs_page);
            if op < 0.999 || bm.is_some() || href.is_some() {
                let b = FixedBox {
                    x: 0.0,
                    y: 0.0,
                    width: clip.width(),
                    height: clip.height(),
                    matrix: None,
                    background: None,
                    border: None,
                    border_radius: None,
                    opacity: op,
                    blend_mode: bm,
                    clip: false,
                    href,
                    children: group_children,
                };
                out.push(FixedElement::Box(b));
            } else {
                out.extend(group_children);
            }
            return;
        }

        // Nested Paste-Into children: render inside the container if rectangular, else after frame.
        let mut nested = Vec::new();
        if it.has_nested_items() {
            let inner = it.inner_bounds();
            let local_to_box = Affine::translate((-inner.x0, -inner.y0));
            let big_clip = Rect::new(-1e6, -1e6, 1e6, 1e6);
            for child in it.shown_children() {
                self.emit_item(
                    child,
                    Affine::IDENTITY,
                    local_to_box,
                    big_clip,
                    abs_page,
                    parent_page,
                    &mut nested,
                );
            }
        }

        match &it.content {
            Content::Graphic(g) => {
                if let Some(el) = self.build_graphic(it, g, m, abs_page, nested) {
                    out.push(el);
                }
            }
            Content::Text(tf) => {
                self.emit_text_frame(it, tf, xf, to_page, clip, abs_page, parent_page, nested, out);
            }
            Content::Unassigned | Content::Group { .. } => {
                self.emit_shape(it, m, abs_page, nested, out);
            }
        }
    }

    fn item_href(&self, it: &Item, abs_page: usize) -> Option<String> {
        if let Some(h) = self.item_links.get(&it.id) {
            return Some(h.clone());
        }
        let act = it.button.as_ref()?;
        let global_cur = self.page_offset + abs_page;
        match act {
            ButtonAction::GoToUrl { url } if !url.is_empty() => Some(url.clone()),
            ButtonAction::GoToPage { page } => {
                Some(format!("page{}.xhtml", self.page_offset + page + 1))
            }
            ButtonAction::GoToFirstPage => Some("page1.xhtml".into()),
            ButtonAction::GoToLastPage => Some(format!("page{}.xhtml", self.total_pages)),
            ButtonAction::GoToNextPage => {
                let target = (global_cur + 2).min(self.total_pages);
                Some(format!("page{target}.xhtml"))
            }
            ButtonAction::GoToPreviousPage => {
                let target = global_cur.max(1);
                Some(format!("page{target}.xhtml"))
            }
            ButtonAction::GoToUrl { .. } => None,
        }
    }

    fn frame_box(&self, it: &Item, m: Affine, abs_page: usize, clip: bool) -> FixedBox {
        let inner = it.inner_bounds();
        let m_div = m * Affine::translate((inner.x0, inner.y0));
        let [a, b, c, d, tx, ty] = m_div.as_coeffs();
        let pure_translate = (a - 1.0).abs() < 1e-4
            && b.abs() < 1e-5
            && c.abs() < 1e-5
            && (d - 1.0).abs() < 1e-4;
        let (x, y, matrix) = if pure_translate {
            (tx, ty, None)
        } else {
            (0.0, 0.0, Some([a, b, c, d, tx, ty]))
        };
        FixedBox {
            x,
            y,
            width: inner.width().max(0.0),
            height: inner.height().max(0.0),
            matrix,
            background: fill_css(self.doc, &it.fill),
            border: border_css(self.doc, &it.stroke),
            border_radius: rounded_corner_css(it),
            opacity: it.opacity.clamp(0.0, 1.0),
            blend_mode: css_blend_mode(it.blend),
            clip,
            href: self.item_href(it, abs_page),
            children: Vec::new(),
        }
    }

    fn emit_shape(
        &mut self,
        it: &Item,
        m: Affine,
        abs_page: usize,
        nested: Vec<FixedElement>,
        out: &mut Vec<FixedElement>,
    ) {
        if can_use_css_box(it) {
            let mut fb = self.frame_box(it, m, abs_page, !nested.is_empty());
            fb.children = nested;
            if fb.background.is_some()
                || fb.border.is_some()
                || fb.href.is_some()
                || !fb.children.is_empty()
            {
                out.push(FixedElement::Box(fb));
            }
            return;
        }
        if let Some(svg) = self.build_vector_svg(it, m, abs_page) {
            out.push(FixedElement::Svg(svg));
        }
        if !nested.is_empty() {
            let mut fb = self.frame_box(it, m, abs_page, true);
            fb.background = None;
            fb.border = None;
            fb.children = nested;
            out.push(FixedElement::Box(fb));
        }
    }

    fn build_vector_svg(&mut self, it: &Item, m: Affine, abs_page: usize) -> Option<FixedVectorSvg> {
        let bp = if it.corners.is_none() {
            it.path.to_bezpath()
        } else {
            corners::apply(&it.path, &it.corners)
        };
        let mut pb = bp;
        pb.apply_affine(m);
        let pad = it.stroke.extent() + 1.0;
        let b = pb.bounding_box().inflate(pad, pad);
        if !b.x0.is_finite() || !b.y0.is_finite() {
            return None;
        }
        let path_d = bezpath_to_svg_d(&pb);
        if path_d.is_empty() {
            return None;
        }

        let mut gradient_defs = String::new();
        let fill = if it.path.is_closed() && !it.fill.is_none() {
            if let Some(grad) = resolve_gradient(&self.doc.swatches, &it.fill.swatch) {
                self.svg_grad_seq += 1;
                let gid = format!("dcg{}", self.svg_grad_seq);
                write_svg_gradient(
                    &gid,
                    &grad,
                    &it.fill,
                    it.inner_bounds(),
                    m,
                    &mut gradient_defs,
                );
                format!("url(#{gid})")
            } else {
                css_color(self.doc, &it.fill.swatch, it.fill.tint).unwrap_or_else(|| "none".into())
            }
        } else {
            "none".into()
        };

        let stroke_scale = {
            let [a, b_c, c, d, _, _] = m.as_coeffs();
            ((a.hypot(b_c) + c.hypot(d)) * 0.5).max(0.0)
        };
        let (stroke, stroke_width, stroke_dasharray) = if !it.stroke.is_none() && it.stroke.weight > 0.0 {
            let color = if let Some(grad) = resolve_gradient(&self.doc.swatches, &it.stroke.swatch) {
                self.svg_grad_seq += 1;
                let gid = format!("dcg{}", self.svg_grad_seq);
                write_svg_gradient(
                    &gid,
                    &grad,
                    &it.stroke.as_fill(),
                    it.inner_bounds(),
                    m,
                    &mut gradient_defs,
                );
                Some(format!("url(#{gid})"))
            } else {
                css_color(self.doc, &it.stroke.swatch, it.stroke.tint)
            };
            let kind = self.doc.stroke_kind(&it.stroke.kind);
            let dash = match kind {
                StrokeType::Dashed { pattern } if pattern.iter().any(|v| *v > 0.0) => Some(
                    pattern
                        .iter()
                        .map(|v| format!("{:.2}", (v * stroke_scale).max(0.1)))
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                StrokeType::Dotted => {
                    let w = (it.stroke.weight * stroke_scale).max(0.5);
                    Some(format!("0.1,{:.2}", w * 2.0))
                }
                _ => None,
            };
            (color, it.stroke.weight * stroke_scale, dash)
        } else {
            (None, 0.0, None)
        };

        if fill == "none" && stroke.is_none() && self.item_href(it, abs_page).is_none() {
            return None;
        }

        let vw = b.width().max(1.0);
        let vh = b.height().max(1.0);
        Some(FixedVectorSvg {
            x: b.x0,
            y: b.y0,
            width: vw,
            height: vh,
            view_box: [b.x0, b.y0, vw, vh],
            matrix: None,
            path_d,
            fill,
            gradient_defs,
            stroke,
            stroke_width,
            stroke_dasharray,
            stroke_linecap: match it.stroke.cap {
                Cap::Butt => "butt",
                Cap::Round => "round",
                Cap::Projecting => "square",
            },
            stroke_linejoin: match it.stroke.join {
                Join::Miter => "miter",
                Join::Round => "round",
                Join::Bevel => "bevel",
            },
            opacity: f64::from(it.opacity.clamp(0.0, 1.0)),
            blend_mode: css_blend_mode(it.blend),
            href: self.item_href(it, abs_page),
        })
    }

    fn build_graphic(
        &mut self,
        it: &Item,
        g: &Graphic,
        m: Affine,
        abs_page: usize,
        nested: Vec<FixedElement>,
    ) -> Option<FixedElement> {
        let g = if it.media.is_some() {
            it.drawn_graphic(&self.doc.assets).unwrap_or_else(|| g.clone())
        } else {
            g.clone()
        };
        let Some(asset) = self.doc.assets.get(&g.asset).cloned() else {
            let mut out = Vec::new();
            self.emit_shape(it, m, abs_page, nested, &mut out);
            return out.into_iter().next();
        };
        if asset.data.is_empty() {
            self.warn(format!("`{}` is missing (linked file not found); left out", asset.name));
            let mut out = Vec::new();
            self.emit_shape(it, m, abs_page, nested, &mut out);
            return out.into_iter().next();
        }
        if it.alt_text.trim().is_empty() {
            self.warn(format!(
                "{}: no alt text (Object Export Options); exported as decorative (alt=\"\")",
                asset.name
            ));
        }
        let Some((mime, bytes)) = raster_or_svg_bytes(&asset) else {
            self.warn(format!("`{}` could not be decoded for EPUB; left out", asset.name));
            return None;
        };

        let inner = it.inner_bounds();
        let mut frame = self.frame_box(it, m, abs_page, true);
        frame.children = nested;
        if !can_use_css_box(it) && it.shape == Shape::Oval && it.corners.is_none() {
            frame.border_radius = Some("50%".into());
        }

        // Graphic space `(0, 0)..(g.size.0, g.size.1)` -> frame box `(0, 0)..(inner.width(), inner.height())`.
        let g_to_box = Affine::translate((-inner.x0, -inner.y0)) * g.xf;
        let [a, b, c, d, tx, ty] = g_to_box.as_coeffs();
        let axis = b.abs() < 1e-6 && c.abs() < 1e-6 && a > 0.0 && d > 0.0;
        let (img_x, img_y, img_width, img_height, img_matrix) = if axis {
            (tx, ty, (g.size.0 * a).max(0.0), (g.size.1 * d).max(0.0), None)
        } else {
            (0.0, 0.0, g.size.0.max(0.0), g.size.1.max(0.0), Some([a, b, c, d, tx, ty]))
        };

        Some(FixedElement::Image(FixedRaster {
            frame,
            img_x,
            img_y,
            img_width,
            img_height,
            img_matrix,
            asset_key: format!("asset-{}-{}", asset.id.0, asset.page),
            mime,
            data: bytes,
            alt: it.alt_text.clone(),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_text_frame(
        &mut self,
        it: &Item,
        tf: &TextFrame,
        xf: Affine,
        to_page: Affine,
        clip: Rect,
        abs_page: usize,
        parent_page: Option<usize>,
        nested: Vec<FixedElement>,
        out: &mut Vec<FixedElement>,
    ) {
        let m = to_page * xf;
        let cs = self.cache.get(self.doc, tf.story, parent_page);
        let Some(ft) = cs.frame(it.id) else {
            self.emit_shape(it, m, abs_page, nested, out);
            return;
        };

        // Non-rectangular frame background/stroke goes behind as `<svg>`.
        let use_box = can_use_css_box(it);
        if !use_box
            && let Some(svg) = self.build_vector_svg(it, m, abs_page)
        {
            out.push(FixedElement::Svg(svg));
        }

        let inner = it.inner_bounds();
        let origin = inner.origin().to_vec2();
        let mut frame = self.frame_box(it, m, abs_page, false);
        if !use_box {
            frame.background = None;
            frame.border = None;
            frame.border_radius = None;
        }
        frame.children = nested;

        let mut rules = Vec::new();
        let mut spans = Vec::new();
        let story_links = self.text_links.get(&tf.story).cloned().unwrap_or_default();
        self.collect_frame_text(&cs, ft, origin, &story_links, &mut rules, &mut spans);

        // Footnotes composed inside this frame.
        for note in &ft.notes {
            if let Some(nft) = note.text.frames.first() {
                let note_offset = origin - note.origin.to_vec2();
                self.collect_frame_text(&note.text, nft, note_offset, &[], &mut rules, &mut spans);
            }
        }

        // Tables placed inside this frame: cell fills, borders, and cell text!
        for tbl in &ft.tables {
            for cell in &tbl.cells {
                if let Some((ref sw, tint)) = cell.fill
                    && let Some(bg) = css_color(self.doc, sw, tint)
                {
                    let r = cell.rect - origin;
                    rules.push(FixedBox {
                        x: r.x0,
                        y: r.y0,
                        width: r.width().max(0.0),
                        height: r.height().max(0.0),
                        background: Some(bg),
                        opacity: 1.0,
                        ..Default::default()
                    });
                }
                if let Some(cft) = cell.text.frames.first() {
                    let cell_offset = origin - cell.origin.to_vec2();
                    self.collect_frame_text(&cell.text, cft, cell_offset, &[], &mut rules, &mut spans);
                }
            }
            for seg in &tbl.strokes {
                if seg.stroke.weight <= 0.0 || seg.stroke.color == "[None]" {
                    continue;
                }
                let Some(sc) = css_color(self.doc, &seg.stroke.color, seg.stroke.tint) else {
                    continue;
                };
                let a = seg.a - origin;
                let b = seg.b - origin;
                let hw = seg.stroke.weight * 0.5;
                let rect = Rect::from_points(a, b).inflate(
                    if (a.x - b.x).abs() < 1e-3 { hw } else { 0.0 },
                    if (a.y - b.y).abs() < 1e-3 { hw } else { 0.0 },
                );
                rules.push(FixedBox {
                    x: rect.x0,
                    y: rect.y0,
                    width: rect.width().max(0.5),
                    height: rect.height().max(0.5),
                    background: Some(sc),
                    opacity: 1.0,
                    ..Default::default()
                });
            }
        }

        out.push(FixedElement::TextFrame { frame, rules, spans });

        // Anchored objects placed in this frame's lines.
        if let Some(st) = self.doc.story(tf.story) {
            let anchored_items: Vec<_> = ft
                .objects
                .iter()
                .filter_map(|o| st.objects.get(o.index).map(|obj| (obj.item.clone(), o.origin)))
                .collect();
            for (aitem, aorigin) in anchored_items {
                self.emit_item(
                    &aitem,
                    xf * Affine::translate(aorigin.to_vec2()),
                    to_page,
                    clip,
                    abs_page,
                    parent_page,
                    out,
                );
            }
        }
    }

    fn collect_frame_text(
        &mut self,
        cs: &ComposedStory,
        ft: &FrameText,
        origin: designcraft_geom::Vec2,
        links: &[(usize, usize, String)],
        rules: &mut Vec<FixedBox>,
        spans: &mut Vec<FixedTextSpan>,
    ) {
        for deco in &ft.decos {
            if deco.nonprinting {
                continue;
            }
            let Some(color) = css_color(self.doc, &deco.color, deco.tint) else {
                continue;
            };
            let r = deco.rect - origin;
            rules.push(FixedBox {
                x: r.x0,
                y: r.y0,
                width: r.width().max(0.0),
                height: r.height().max(0.5),
                background: Some(color),
                opacity: 1.0,
                ..Default::default()
            });
        }

        for line in &ft.lines {
            let mut cur: Option<(FixedTextSpan, f64, u32)> = None;
            for g in &line.glyphs {
                if !g.visible {
                    continue;
                }
                let text_piece: String = match &g.generated_text {
                    Some(gt) => gt.chars().filter(|c| !is_private_marker(*c)).collect(),
                    None => {
                        if is_private_marker(g.rendered_char) {
                            String::new()
                        } else {
                            g.rendered_char.to_string()
                        }
                    }
                };
                if text_piece.is_empty() {
                    continue;
                }

                let rs = cs.styles.get(g.style as usize);
                let font_size = rs.map_or(12.0, |s| s.size).max(1.0);
                let color = rs
                    .and_then(|s| css_color(self.doc, &s.fill, s.fill_tint))
                    .unwrap_or_else(|| "#000000".into());
                let (underline, strikethrough) =
                    rs.map_or((false, false), |s| (s.underline, s.strikethrough));
                let href = links
                    .iter()
                    .find(|(s, e, _)| g.byte >= *s && g.byte < *e)
                    .map(|(_, _, h)| h.clone());

                let family = g.face.family.clone();
                let style_name = g.face.style.clone();
                self.used_faces.insert((family.clone(), style_name.clone()));

                let gx = g.x - origin.x;
                let gy = (line.baseline + g.y - line.ascent.max(font_size * 0.8)) - origin.y;

                let can_merge = if let Some((prev, expected_x, prev_style)) = &cur {
                    *prev_style == g.style
                        && !g.rtl
                        && !prev.rtl
                        && prev.font_family == family
                        && prev.font_style == style_name
                        && prev.href == href
                        && (prev.y - gy).abs() < 0.25
                        && (gx - *expected_x).abs() < 0.45
                } else {
                    false
                };

                if can_merge {
                    if let Some((prev, expected_x, _)) = &mut cur {
                        prev.text.push_str(&text_piece);
                        *expected_x = gx + g.adv;
                    }
                } else {
                    if let Some((finished, _, _)) = cur.take() {
                        spans.push(finished);
                    }
                    cur = Some((
                        FixedTextSpan {
                            x: gx,
                            y: gy,
                            text: text_piece,
                            font_family: family,
                            font_style: style_name,
                            font_size,
                            color,
                            letter_spacing: 0.0,
                            underline,
                            strikethrough,
                            rtl: g.rtl,
                            href,
                        },
                        gx + g.adv,
                        g.style,
                    ));
                }
            }
            if let Some((finished, _, _)) = cur.take() {
                spans.push(finished);
            }
        }
    }
}

fn is_private_marker(c: char) -> bool {
    ('\u{E000}'..='\u{F8FF}').contains(&c) || c == '\u{2028}' || c == '\n' || c == '\r'
}

fn collect_anchor_pages(doc: &Document, cache: &Cache) -> HashMap<u64, usize> {
    let mut out = HashMap::new();
    for (sid, st) in &doc.stories {
        if st.anchors.is_empty() {
            continue;
        }
        let cs = cache.get(doc, *sid, None);
        for (idx, (byte_pos, _)) in st.text.match_indices(ANCHOR_MARK).enumerate() {
            let Some(anchor) = st.anchors.get(idx) else {
                continue;
            };
            let page = cs
                .frames
                .iter()
                .find(|ft| byte_pos >= ft.range.start && byte_pos <= ft.range.end)
                .or_else(|| cs.frames.first())
                .and_then(|ft| doc.page_of_item(ft.frame));
            if let Some(p) = page {
                out.insert(anchor.id, p);
            }
        }
    }
    out
}

fn dest_href(
    dest: &HyperlinkDest,
    anchor_pages: &HashMap<u64, usize>,
    page_offset: usize,
) -> Option<String> {
    match dest {
        HyperlinkDest::Url(u) => (!u.trim().is_empty()).then(|| u.clone()),
        HyperlinkDest::Email(e) => (!e.trim().is_empty()).then(|| {
            if e.starts_with("mailto:") {
                e.clone()
            } else {
                format!("mailto:{e}")
            }
        }),
        HyperlinkDest::Page(p) => Some(format!("page{}.xhtml", page_offset + p + 1)),
        HyperlinkDest::Anchor(a) => anchor_pages
            .get(a)
            .map(|p| format!("page{}.xhtml", page_offset + p + 1)),
    }
}

fn css_color(doc: &Document, swatch: &str, tint: f32) -> Option<String> {
    if swatch == designcraft_color::swatch::NONE || swatch.is_empty() {
        return None;
    }
    let c = doc.resolve_color(swatch, tint)?;
    let [r, g, b, a] = c.to_rgba8(1.0);
    if a == 0 {
        return None;
    }
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}

fn fill_css(doc: &Document, fill: &Fill) -> Option<String> {
    if fill.is_none() {
        return None;
    }
    if let Some(grad) = resolve_gradient(&doc.swatches, &fill.swatch) {
        let stops: Vec<String> = grad
            .expanded_stops()
            .into_iter()
            .map(|(off, col, alpha)| {
                let [r, g, b, a] = col.to_rgba8(alpha);
                if a < 255 {
                    format!("rgba({r},{g},{b},{:.3}) {:.1}%", f64::from(a) / 255.0, off * 100.0)
                } else {
                    format!("#{r:02x}{g:02x}{b:02x} {:.1}%", off * 100.0)
                }
            })
            .collect();
        if stops.is_empty() {
            return None;
        }
        return Some(match grad.kind {
            GradientKind::Radial => format!("radial-gradient(ellipse at center, {})", stops.join(", ")),
            GradientKind::Linear | GradientKind::Freeform => {
                let css_deg = 90.0 - fill.gradient_angle.unwrap_or(0.0);
                format!("linear-gradient({css_deg:.1}deg, {})", stops.join(", "))
            }
        });
    }
    css_color(doc, &fill.swatch, fill.tint)
}

fn border_css(doc: &Document, stroke: &Stroke) -> Option<String> {
    if stroke.is_none() || stroke.weight <= 0.0 {
        return None;
    }
    let color = css_color(doc, &stroke.swatch, stroke.tint)?;
    let style = match doc.stroke_kind(&stroke.kind) {
        StrokeType::Dashed { .. } => "dashed",
        StrokeType::Dotted => "dotted",
        StrokeType::ThinThin | StrokeType::ThickThick => "double",
        _ => "solid",
    };
    Some(format!("{:.2}px {style} {color}", stroke.weight))
}

fn can_use_css_box(it: &Item) -> bool {
    if it.shape != Shape::Rectangle || !it.path.is_closed() {
        return false;
    }
    let bp = it.path.to_bezpath();
    let inner = bp.bounding_box();
    if !is_exact_rect(&bp, inner) {
        return false;
    }
    it.corners
        .corners
        .iter()
        .all(|c| c.is_none() || c.shape == CornerShape::Rounded)
}

fn rounded_corner_css(it: &Item) -> Option<String> {
    if it.corners.is_none() {
        return None;
    }
    let radii: [f64; 4] = std::array::from_fn(|i| {
        let c = it.corners.corners[i];
        if c.shape == CornerShape::Rounded && c.size > 0.0 {
            c.size
        } else {
            0.0
        }
    });
    if radii.iter().all(|r| *r <= 0.0) {
        return None;
    }
    Some(format!(
        "{:.2}px {:.2}px {:.2}px {:.2}px",
        radii[0], radii[1], radii[2], radii[3]
    ))
}

fn is_exact_rect(bp: &BezPath, r: Rect) -> bool {
    let mut pts = Vec::new();
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => pts.push(p),
            PathEl::ClosePath => {}
            _ => return false,
        }
    }
    if pts.len() == 5 && pts.first().zip(pts.last()).is_some_and(|(a, b)| (*a - *b).hypot() < 1e-6)
    {
        pts.pop();
    }
    pts.len() == 4
        && pts.iter().all(|p| {
            ((p.x - r.x0).abs() < 1e-6 || (p.x - r.x1).abs() < 1e-6)
                && ((p.y - r.y0).abs() < 1e-6 || (p.y - r.y1).abs() < 1e-6)
        })
}

fn css_blend_mode(bm: BlendMode) -> Option<&'static str> {
    Some(match bm {
        BlendMode::Normal => return None,
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft-light",
        BlendMode::HardLight => "hard-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    })
}

fn bezpath_to_svg_d(bp: &BezPath) -> String {
    let mut d = String::new();
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(Point { x, y }) => {
                let _ = write!(d, "M{x:.2} {y:.2}");
            }
            PathEl::LineTo(Point { x, y }) => {
                let _ = write!(d, "L{x:.2} {y:.2}");
            }
            PathEl::QuadTo(p1, p2) => {
                let _ = write!(d, "Q{:.2} {:.2} {:.2} {:.2}", p1.x, p1.y, p2.x, p2.y);
            }
            PathEl::CurveTo(p1, p2, p3) => {
                let _ = write!(
                    d,
                    "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                    p1.x, p1.y, p2.x, p2.y, p3.x, p3.y
                );
            }
            PathEl::ClosePath => d.push('Z'),
        }
    }
    d
}

fn write_svg_gradient(
    id: &str,
    grad: &designcraft_color::Gradient,
    fill: &Fill,
    inner_bounds: Rect,
    m: Affine,
    defs: &mut String,
) {
    let mut stops_xml = String::new();
    for (off, col, alpha) in grad.expanded_stops() {
        let [r, g, b, a] = col.to_rgba8(alpha);
        let _ = write!(
            stops_xml,
            "<stop offset=\"{:.1}%\" stop-color=\"#{r:02x}{g:02x}{b:02x}\" stop-opacity=\"{:.3}\"/>",
            off * 100.0,
            f64::from(a) / 255.0
        );
    }
    match grad.kind {
        GradientKind::Radial => {
            let (c, rx, ry) = fill.radial_ellipse(inner_bounds);
            let cp = m * c;
            let [a, b, c2, d, _, _] = m.as_coeffs();
            let r = ((rx * a.hypot(b)) + (ry * c2.hypot(d))) * 0.5;
            let _ = write!(
                defs,
                "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\">{stops_xml}</radialGradient>",
                cp.x,
                cp.y,
                r.max(1.0)
            );
        }
        GradientKind::Linear | GradientKind::Freeform => {
            let (p0, p1) = fill.linear_axis(inner_bounds);
            let q0 = m * p0;
            let q1 = m * p1;
            let _ = write!(
                defs,
                "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\">{stops_xml}</linearGradient>",
                q0.x, q0.y, q1.x, q1.y
            );
        }
    }
}

fn raster_or_svg_bytes(asset: &designcraft_doc::Asset) -> Option<(String, Vec<u8>)> {
    let data: &[u8] = &asset.data;
    let detected = designcraft_images::mime(data);
    match detected {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/svg+xml" => {
            return Some((detected.to_string(), data.to_vec()));
        }
        _ => {}
    }
    if asset.mime == "application/pdf" || designcraft_render::is_pdf(data) {
        let png = designcraft_render::pdf_page_png_checked(data, asset.page as usize, 1440, &[], None).ok()?;
        return Some(("image/png".into(), png));
    }
    if designcraft_images::is_eps(data) {
        let png = designcraft_images::eps_png(data)?;
        return Some(("image/png".into(), png));
    }
    let img = designcraft_images::decode_rgba(data)?;
    let (w, h) = img.dimensions();
    let mut premul = img.into_raw();
    for px in premul.chunks_exact_mut(4) {
        let a = u32::from(px[3]);
        if a < 255 {
            px[0] = ((u32::from(px[0]) * a + 127) / 255) as u8;
            px[1] = ((u32::from(px[1]) * a + 127) / 255) as u8;
            px[2] = ((u32::from(px[2]) * a + 127) / 255) as u8;
        }
    }
    let png = designcraft_render::Rendered { pixels: premul, width: w, height: h }.to_png();
    (!png.is_empty()).then_some(("image/png".into(), png))
}
