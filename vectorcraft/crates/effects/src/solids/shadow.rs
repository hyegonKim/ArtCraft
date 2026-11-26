//! Casts hard-edged vector shadows behind or below 3D solids and revolved surfaces.

use std::ops::Range;
use std::sync::Arc;

use serde_json::Value;
use vectorcraft_doc::color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Appearance, Node, NodeId};
use vectorcraft_three_d::{Shadow, ShadowCaster, ShadowPosition};

use super::{Scope, composites};
use crate::util::{flag, num, text};

/// Opacity and blending mode of the fill or stroke casting a shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ink {
    pub(crate) opacity: f32,
    pub(crate) blend: BlendMode,
}

/// One solid's shadow contribution: its member's scopes, its ink, and its face range in the caster.
pub(crate) type Cast<'a> = (&'a [Scope], Ink, Range<usize>);

/// Validates the shadow parameters on a 3D effect's JSON parameter object.
pub(crate) fn validate(p: &Value) -> Result<(), String> {
    if let Some(v) = p.get("shadow")
        && !v.is_boolean()
    {
        return Err("shadow must be true or false".into());
    }
    if let Some(v) = p.get("shadowPosition")
        && v.as_str().and_then(ShadowPosition::parse).is_none()
    {
        return Err("shadowPosition must be behind or below".into());
    }
    for (key, min, max) in [("shadowDistance", 0.0, 100.0), ("shadowBounds", 10.0, 400.0)] {
        if let Some(v) = p.get(key) {
            let Some(d) = v.as_f64().filter(|d| d.is_finite()) else {
                return Err(format!("{key} must be a number from {min} to {max}"));
            };
            if !(min..=max).contains(&d) {
                return Err(format!("{key} must be a number from {min} to {max}"));
            }
        }
    }
    Ok(())
}

/// Parses active [`Shadow`] settings from `p`, or `None` when `shadow` is false.
pub(crate) fn options(p: &Value) -> Option<Shadow> {
    if !flag(p, "shadow", false) {
        return None;
    }
    let position = ShadowPosition::parse(text(p, "shadowPosition", "behind")).unwrap_or_default();
    Some(Shadow {
        position,
        distance: (num(p, "shadowDistance", 0.0) / 100.0).clamp(0.0, 1.0),
        bounds: (num(p, "shadowBounds", 100.0) / 100.0).clamp(0.1, 4.0),
    })
}

fn shadow_node(id: NodeId, path: vectorcraft_geom::PathData, ink: Ink) -> Node {
    let mut node = Node::path(
        id,
        path,
        Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0),
    );
    node.opacity = (0.5 * ink.opacity).clamp(0.0, 1.0);
    node.blend = ink.blend;
    node
}

/// Builds a single shadow path node from `caster` with `settings` and `ink`.
pub(crate) fn art(id: NodeId, caster: &ShadowCaster, settings: Shadow, ink: Ink) -> Option<Node> {
    if ink.opacity <= 0.0 {
        return None;
    }
    let path = caster.path(settings)?;
    Some(shadow_node(id, path, ink))
}

/// Builds the shadow nodes for `cast` on one shared shadow plane, grouping runs that share the
/// same scopes and ink, and wrapping in member opacity masks/blending scopes when `masked`.
pub(crate) fn scoped_art(
    id: NodeId,
    caster: &ShadowCaster,
    settings: Shadow,
    cast: Vec<Cast<'_>>,
    masked: bool,
) -> Vec<Arc<Node>> {
    let all_ranges: Vec<Range<usize>> = cast.iter().map(|(_, _, r)| r.clone()).filter(|r| !r.is_empty()).collect();
    if all_ranges.is_empty() {
        return Vec::new();
    }
    let Some(plane) = caster.shadow_plane(settings, &all_ranges) else {
        return Vec::new();
    };
    type ShadowRun<'b> = (&'b [Scope], Ink, Vec<Range<usize>>);
    let mut runs: Vec<ShadowRun<'_>> = Vec::new();
    for (scopes, ink, range) in cast {
        if range.is_empty() || ink.opacity <= 0.0 {
            continue;
        }
        let effective_scopes = if masked { scopes } else { &[] };
        if let Some(last) = runs.last_mut()
            && last.0 == effective_scopes
            && (last.1.opacity - ink.opacity).abs() <= 1e-6
            && last.1.blend == ink.blend
        {
            last.2.push(range);
        } else {
            runs.push((effective_scopes, ink, vec![range]));
        }
    }
    let built: Vec<(&[Scope], Arc<Node>)> = runs
        .into_iter()
        .filter_map(|(scopes, ink, ranges)| {
            let path = caster.path_on(&plane, &ranges)?;
            Some((scopes, Arc::new(shadow_node(id, path, ink))))
        })
        .collect();
    if !masked {
        return built.into_iter().map(|(_, n)| n).collect();
    }
    let mask_scopes: Vec<(Vec<Scope>, Arc<Node>)> = built
        .into_iter()
        .map(|(scopes, n)| {
            let filtered = scopes
                .iter()
                .filter(|s| composites(s))
                .map(|s| Scope { opacity: 1.0, ..s.clone() })
                .collect();
            (filtered, n)
        })
        .collect();
    crate::revolve::scoped(
        id,
        mask_scopes.iter().map(|(sc, n)| (sc.as_slice(), n.clone())).collect(),
        0,
    )
}
