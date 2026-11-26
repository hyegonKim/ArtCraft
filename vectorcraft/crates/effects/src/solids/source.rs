//! Resolves symbol instances, blends, and clipping groups into plain vector trees that 3D effects
//! can extrude, inflate, rotate, or revolve.

use std::borrow::Cow;
use std::sync::Arc;

use vectorcraft_doc::{Appearance, AppearanceItem, Document, Node, NodeKind};
use vectorcraft_geom::{FillRule, PathData};
use vectorcraft_pathops::{BoolOp, boolean};

use crate::{GeomContext, apply_geometry_with};

const MAX_DEPTH: usize = 8;

/// Whether `n` is a symbol instance, blend, or clipping group that resolves into vector members.
pub(crate) fn resolves(n: &Node) -> bool {
    matches!(
        n.kind,
        NodeKind::SymbolInstance { .. } | NodeKind::Blend { .. } | NodeKind::Group { clip: true, .. }
    )
}

/// Refusal message when a raster image is given to a 3D effect.
pub(crate) fn image_refusal(label: &str) -> String {
    format!("{label} can't build a solid from an image: trace it first with Object › Image Trace › Make.")
}

/// Resolves `n` (and any nested instances, blends, or clipping groups inside it) into plain vector
/// paths/groups so a 3D effect can build solids from it.
pub(crate) fn resolve<'a>(doc: Option<&'a Document>, n: &'a Node, label: &str) -> Result<Cow<'a, Node>, String> {
    resolve_for(doc, n, label, false)
}

/// [`resolve`] for plane (`Rotate`) or solid/revolve 3D effects.
pub(crate) fn resolve_for<'a>(
    doc: Option<&'a Document>,
    n: &'a Node,
    label: &str,
    plane: bool,
) -> Result<Cow<'a, Node>, String> {
    resolve_depth(doc, n, label, plane, 0)
}

fn tree_needs_resolve(n: &Node) -> bool {
    if resolves(n) {
        return true;
    }
    n.children().is_some_and(|ch| ch.iter().any(|c| tree_needs_resolve(c)))
}

#[allow(clippy::only_used_in_recursion)]
fn resolve_depth<'a>(
    doc: Option<&'a Document>,
    n: &'a Node,
    label: &str,
    plane: bool,
    depth: usize,
) -> Result<Cow<'a, Node>, String> {
    if !tree_needs_resolve(n) {
        return Ok(Cow::Borrowed(n));
    }
    if depth >= MAX_DEPTH {
        return Err(format!("{} can't build solids from art nested more than {MAX_DEPTH} levels deep", label));
    }
    match &n.kind {
        NodeKind::SymbolInstance { symbol, xf } => {
            let Some(sym) = doc.and_then(|d| d.symbols.iter().find(|s| s.name == *symbol)) else {
                return Err(format!("{label} can't build a solid from the instance of symbol \"{symbol}\": the symbol's art isn't available"));
            };
            let Some(doc) = doc else {
                return Err(format!("{label} can't build a solid from the instance of symbol \"{symbol}\": the symbol's art isn't available"));
            };
            let art = vectorcraft_doc::symbol_overrides::dynamic_art(&sym.art, n);
            let Some(art) = doc.place_symbol_art(symbol, &art, *xf) else {
                return Err(format!("{} can't build solids from this art: its symbol instances, blends and clipping groups hold too many objects", label));
            };
            let inner = resolve_depth(Some(doc), &art, label, plane, depth + 1)?.into_owned();
            let mut out = n.clone();
            out.kind = match inner.kind {
                NodeKind::Group { children, clip: false } => NodeKind::Group { children, clip: false },
                _ => NodeKind::Group { children: vec![Arc::new(inner)], clip: false },
            };
            Ok(Cow::Owned(out))
        }
        NodeKind::Blend { .. } => {
            let expanded = crate::expanded_live_group(doc, n);
            let inner = resolve_depth(doc, &expanded, label, plane, depth + 1)?.into_owned();
            let mut out = n.clone();
            out.kind = inner.kind;
            Ok(Cow::Owned(out))
        }
        NodeKind::Group { children, clip: true } => {
            let Some(clip_node) = children.first() else {
                return Err(format!("{}: the clipping path is too complex to cut the art with", label));
            };
            let Some((mask_bez, mask_rule)) = crate::clip_outline(clip_node) else {
                return Err(format!("{}: the clipping path is too complex to cut the art with", label));
            };
            let mask_path = PathData::from_bezpath(&mask_bez);
            let mut clipped_children = Vec::new();
            for member in children.iter().skip(1) {
                if !member.visible || member.is_guide() {
                    continue;
                }
                let resolved = resolve_depth(doc, member, label, plane, depth + 1)?;
                clip_member(&resolved, (&mask_path, mask_rule), label, 0, &mut clipped_children)?;
            }
            let mut out = n.clone();
            out.kind = NodeKind::Group { children: clipped_children, clip: false };
            Ok(Cow::Owned(out))
        }
        NodeKind::Group { children, clip: false } => {
            let mut next = Vec::with_capacity(children.len());
            for c in children {
                next.push(Arc::new(resolve_depth(doc, c, label, plane, depth + 1)?.into_owned()));
            }
            let mut out = n.clone();
            out.kind = NodeKind::Group { children: next, clip: false };
            Ok(Cow::Owned(out))
        }
        NodeKind::Layer { children, color, template, printable, preview, dim_images, clip } => {
            let mut next = Vec::with_capacity(children.len());
            for c in children {
                next.push(Arc::new(resolve_depth(doc, c, label, plane, depth + 1)?.into_owned()));
            }
            let mut out = n.clone();
            out.kind = NodeKind::Layer {
                children: next,
                color: *color,
                template: *template,
                printable: *printable,
                preview: *preview,
                dim_images: *dim_images,
                clip: *clip,
            };
            Ok(Cow::Owned(out))
        }
        _ => Ok(Cow::Borrowed(n)),
    }
}

fn clip_member(
    member: &Node,
    (mask, mask_rule): (&PathData, FillRule),
    label: &str,
    clip_depth: usize,
    out: &mut Vec<Arc<Node>>,
) -> Result<(), String> {
    const MAX_DEPTH: usize = 64;
    if clip_depth >= MAX_DEPTH {
        return Err(format!("{} can't build solids from art nested more than {MAX_DEPTH} levels deep", label));
    }
    if matches!(member.kind, NodeKind::Image(_)) {
        return Err(image_refusal(label));
    }
    if super::is_container(member) {
        let mut inner = Vec::new();
        for c in member.children().into_iter().flatten() {
            if c.visible && !c.is_guide() {
                clip_member(c, (mask, mask_rule), label, clip_depth + 1, &mut inner)?;
            }
        }
        if !inner.is_empty() {
            let mut g = member.clone();
            g.kind = NodeKind::Group { children: inner, clip: false };
            out.push(Arc::new(g));
        }
        return Ok(());
    }
    let (path, rule) = super::object_profile(member, label)?;
    let ctx = GeomContext::of(member);
    let shaped = apply_geometry_with(
        &member.appearance.effects,
        &path,
        path.bounds().unwrap_or_default(),
        &ctx,
    );
    let paints = super::solid_paints(member);
    let mut parts = Vec::new();
    for item in &paints {
        match item {
            AppearanceItem::Fill(fill) => {
                let inter = boolean(&shaped, rule, mask, mask_rule, BoolOp::Intersect);
                if inter.bounds().is_some() {
                    let app = Appearance { items: vec![AppearanceItem::Fill(fill.clone())], ..Default::default() };
                    parts.push(Arc::new(crate::group::shape_node(member.id, inter, FillRule::NonZero, app)));
                }
            }
            AppearanceItem::Stroke(stroke) if stroke.width > 0.0 && stroke.width.is_finite() => {
                let area = crate::stroke::outline_region(&shaped, rule, stroke);
                let inter = boolean(&area, FillRule::EvenOdd, mask, mask_rule, BoolOp::Intersect);
                if inter.bounds().is_some() {
                    let mut fill = vectorcraft_doc::FillLayer::new(stroke.paint.clone());
                    fill.opacity = stroke.opacity;
                    fill.blend = stroke.blend;
                    let app = Appearance { items: vec![AppearanceItem::Fill(fill)], ..Default::default() };
                    parts.push(Arc::new(crate::group::shape_node(member.id, inter, FillRule::NonZero, app)));
                }
            }
            _ => {}
        }
    }
    match parts.len() {
        0 => {}
        1 => {
            if let Some(mut single) = parts.pop() {
                let m = Arc::make_mut(&mut single);
                m.opacity = member.opacity;
                m.blend = member.blend;
                m.appearance.effects = crate::without_geometry(&member.appearance.effects);
                if let Some(extra_mask) = &member.extras().mask {
                    m.extras_mut().mask = Some(extra_mask.clone());
                }
                out.push(single);
            }
        }
        _ => {
            let mut g = member.clone();
            g.kind = NodeKind::Group { children: parts, clip: false };
            g.appearance = Appearance {
                effects: crate::without_geometry(&member.appearance.effects),
                ..Default::default()
            }
            .into();
            out.push(Arc::new(g));
        }
    }
    Ok(())
}
