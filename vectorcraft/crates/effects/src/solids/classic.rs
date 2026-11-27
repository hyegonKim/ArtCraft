//! Extrude & Bevel (Classic) and Rotate (Classic): parameter validation, surface options, and
//! flat per-facet classic surface shading shared with Revolve (Classic).

use std::sync::Arc;

use serde_json::Value;
use vectorcraft_doc::color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, FillLayer, LineJoin, Node, OpacityMask, StrokeLayer};
use vectorcraft_geom::Rect;
use vectorcraft_three_d::{Bevel, BevelShape, ClassicShading, ClassicSurface, Extrude, Facet};

use crate::util::{flag, num, text};

pub const CLASSIC_EXTRUDE: &str = "threeD.classicExtrude";
pub const CLASSIC_ROTATE: &str = "threeD.classicRotate";

/// Surfaces available on Rotate (Classic).
pub const ROTATE_SURFACES: [&str; 2] = ["none", "diffuse"];
/// Classic shading color modes.
pub const SHADING_COLORS: [&str; 3] = ["black", "custom", "none"];

const EXTRUDE_RANGES: [(&str, f64, f64, bool); 13] = [
    ("depth", 0.0, 2000.0, false),
    ("bevelHeight", 1.0, 100.0, false),
    ("rotationX", -180.0, 180.0, false),
    ("rotationY", -180.0, 180.0, false),
    ("rotationZ", -180.0, 180.0, false),
    ("perspectiveAngle", 0.0, 160.0, false),
    ("lightRotation", -180.0, 180.0, false),
    ("lightHeight", 0.0, 90.0, false),
    ("lightIntensity", 0.0, 100.0, false),
    ("ambient", 0.0, 100.0, false),
    ("highlightIntensity", 0.0, 100.0, false),
    ("highlightSize", 0.0, 100.0, false),
    ("blendSteps", 1.0, 256.0, true),
];

const EXTRUDE_FLAGS: [&str; 4] = ["cap", "bevelInside", "lightBehind", "drawHiddenFaces"];

const ROTATE_RANGES: [(&str, f64, f64, bool); 9] = [
    ("rotationX", -180.0, 180.0, false),
    ("rotationY", -180.0, 180.0, false),
    ("rotationZ", -180.0, 180.0, false),
    ("perspectiveAngle", 0.0, 160.0, false),
    ("lightRotation", -180.0, 180.0, false),
    ("lightHeight", 0.0, 90.0, false),
    ("lightIntensity", 0.0, 100.0, false),
    ("ambient", 0.0, 100.0, false),
    ("blendSteps", 1.0, 256.0, true),
];

const ROTATE_FLAGS: [&str; 1] = ["lightBehind"];

fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::rgb(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0))
}

/// Validates Extrude & Bevel (Classic) parameters.
pub(crate) fn validate(p: &Value) -> Result<(), String> {
    validate_classic(CLASSIC_EXTRUDE, p, &EXTRUDE_RANGES, &EXTRUDE_FLAGS)
}

/// Validates Rotate (Classic) parameters.
pub(crate) fn validate_rotate(p: &Value) -> Result<(), String> {
    validate_classic(CLASSIC_ROTATE, p, &ROTATE_RANGES, &ROTATE_FLAGS)
}

/// Shared parameter validator for Extrude & Bevel (Classic), Revolve (Classic), and Rotate (Classic).
pub(crate) fn validate_classic(id: &str, p: &Value, ranges: &[(&str, f64, f64, bool)], flags: &[&str]) -> Result<(), String> {
    let Some(obj) = p.as_object() else {
        let got = p.to_string();
        return Err(format!("the params must be an object, not {got}"));
    };
    let rotate = id == CLASSIC_ROTATE;
    let extrude = id == CLASSIC_EXTRUDE;
    let revolve = id == crate::CLASSIC_REVOLVE;
    for key in obj.keys() {
        let known = ranges.iter().any(|(k, ..)| *k == key.as_str())
            || flags.contains(&key.as_str())
            || matches!(key.as_str(), "surface" | "shadingColor" | "shadingCustom" | "lights")
            || (extrude && key == "bevel")
            || (revolve && key == "edge");
        if !known {
            let mut valid: Vec<&str> = ranges.iter().map(|&(k, ..)| k).chain(flags.iter().copied()).collect();
            valid.extend(["surface", "shadingColor", "shadingCustom", "lights"]);
            if extrude {
                valid.push("bevel");
            }
            if revolve {
                valid.push("edge");
            }
            let path = key;
            let keys = valid.join(", ");
            return Err(format!("unknown parameter `{path}`; it takes: {keys}"));
        }
    }
    for &(key, min, max, whole) in ranges {
        if let Some(val) = obj.get(key) {
            let Some(x) = val.as_f64().filter(|v| v.is_finite()) else {
                return Err(format!("`{key}` must be a number"));
            };
            if whole && (x.fract() != 0.0 || x < min || x > max) {
                return Err(format!("`{key}` must be a whole number between {min} and {max} (got {x})"));
            }
            if x < min || x > max {
                return Err(format!("`{key}` must be between {min} and {max} (got {x})"));
            }
        }
    }
    for &key in flags {
        if let Some(val) = obj.get(key)
            && !val.is_boolean()
        {
            return Err(format!("`{key}` must be true or false"));
        }
    }
    if let Some(val) = obj.get("surface") {
        let ok = val.as_str().is_some_and(|s| if rotate { ROTATE_SURFACES.contains(&s) } else { ClassicSurface::parse(s).is_some() });
        if !ok {
            let allowed = if rotate { ROTATE_SURFACES.join(", ") } else { "wireframe, none, diffuse, plastic".to_string() };
            return Err(format!("`surface` must be one of {}", allowed));
        }
    }
    if let Some(val) = obj.get("shadingColor")
        && !val.as_str().is_some_and(|s| SHADING_COLORS.contains(&s))
    {
        let key = "shadingColor";
        return Err(format!("`{key}` must be one of {}", SHADING_COLORS.join(", ")));
    }
    if let Some(val) = obj.get("shadingCustom")
        && val.as_str().and_then(parse_hex_color).is_none()
    {
        let key = "shadingCustom";
        return Err(format!("`{key}` must be a #rrggbb color"));
    }
    crate::lighting::validate(p)
}

/// Extrude options built from Extrude & Bevel (Classic) parameters `p`.
pub fn classic_extrude_options(p: &Value) -> Extrude {
    let surface = text(p, "surface", "plastic");
    let bevel_h = num(p, "bevelHeight", 4.0);
    Extrude {
        depth: num(p, "depth", 50.0),
        twist: 0.0,
        taper: 1.0,
        cap: flag(p, "cap", true),
        bevel: BevelShape::parse(text(p, "bevel", "none")).map(|shape| Bevel {
            shape,
            width: bevel_h,
            height: bevel_h,
            repeat: 1,
            space: 0.0,
            inside: flag(p, "bevelInside", false),
            both_sides: false,
        }),
        hidden: surface == "wireframe" || flag(p, "drawHiddenFaces", false),
    }
}

/// Classic surface shading settings built from classic 3D parameters `p`.
pub fn classic_shading(p: &Value) -> ClassicShading {
    let default_surface = if p.get("depth").is_none() && p.get("angle").is_none() { "diffuse" } else { "plastic" };
    let surface = ClassicSurface::parse(text(p, "surface", default_surface)).unwrap_or(ClassicSurface::Plastic);
    ClassicShading {
        surface,
        ambient: (num(p, "ambient", 50.0) / 100.0).clamp(0.0, 1.0),
        highlight_intensity: (num(p, "highlightIntensity", 60.0) / 100.0).clamp(0.0, 1.0),
        highlight_size: (num(p, "highlightSize", 90.0) / 100.0).clamp(0.0, 1.0),
        blend_steps: num(p, "blendSteps", 25.0).round().clamp(1.0, 256.0) as u32,
    }
}

fn classic_lights(p: &Value) -> Vec<([f64; 3], f64)> {
    if let Some(lights) = crate::lighting::lights(p) {
        return lights.iter().map(|l| (l.direction(), l.intensity)).collect();
    }
    let view = super::view_options(p);
    let r = vectorcraft_three_d::Revolve {
        light_rotation: view.light_rotation,
        light_height: view.light_height,
        light_behind: view.light_behind,
        ..Default::default()
    };
    vec![(r.light_direction(), (num(p, "lightIntensity", 100.0) / 100.0).clamp(0.0, 1.0))]
}

fn shade_target(p: &Value) -> Option<Color> {
    match text(p, "shadingColor", "black") {
        "none" => None,
        "custom" => Some(parse_hex_color(text(p, "shadingCustom", "#000000")).unwrap_or(Color::BLACK)),
        _ => Some(Color::BLACK),
    }
}

fn blend_rgb(base: Color, target: Option<Color>, tone: f64, spec: f64) -> Color {
    let [mut r, mut g, mut b] = base.to_rgb();
    if let Some(shade) = target {
        let [sr, sg, sb] = shade.to_rgb();
        let dark = (1.0 - tone.clamp(0.0, 1.0)) as f32;
        r += (sr - r) * dark;
        g += (sg - g) * dark;
        b += (sb - b) * dark;
    }
    let hi = spec.clamp(0.0, 1.0) as f32;
    r += (1.0 - r) * hi;
    g += (1.0 - g) * hi;
    b += (1.0 - b) * hi;
    Color::rgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

fn classic_seam(paint: Paint, width: f64) -> StrokeLayer {
    let mut s = StrokeLayer::new(paint, width);
    s.join = LineJoin::Round;
    s
}

/// Draws `facets` as one classic 3D surface group (`Wireframe`, `NoShading`, `Diffuse`, or `Plastic`).
pub(crate) fn surface(n: &Node, facets: Vec<Facet>, paint: &Paint, opacity: f32, blend: BlendMode, p: &Value) -> Node {
    let shading = classic_shading(p);
    if shading.surface == ClassicSurface::Wireframe {
        let stroke_paint = if matches!(paint, Paint::None) { Paint::solid(Color::BLACK) } else { paint.clone() };
        let children = facets
            .into_iter()
            .map(|face| {
                let mut stroke = StrokeLayer::new(stroke_paint.clone(), 0.5);
                stroke.join = LineJoin::Round;
                let app = Appearance { items: vec![AppearanceItem::Stroke(stroke)], ..Default::default() };
                Arc::new(Node::path(n.id, face.path(), app))
            })
            .collect();
        let mut group = Node::group(n.id, children);
        group.opacity = opacity;
        group.blend = blend;
        return group;
    }
    let (opaque, mask) = if shading.surface == ClassicSurface::NoShading { (paint.clone(), None) } else { crate::lighting::opaque_paint(paint) };
    let lights = classic_lights(p);
    let target = shade_target(p);
    let children = facets
        .into_iter()
        .map(|face| {
            let seam = (face.thickness() / 2.0).clamp(0.05, 0.75);
            let (tone, spec) = shading.tone(face.normal, &lights);
            let mut items = Vec::new();
            if shading.surface == ClassicSurface::NoShading {
                items.push(AppearanceItem::Fill(FillLayer::new(opaque.clone())));
            } else if let Some(c) = opaque.color() {
                let lit = blend_rgb(c, target, tone, spec);
                items.push(AppearanceItem::Fill(FillLayer::new(Paint::solid(lit))));
            } else {
                items.push(AppearanceItem::Fill(FillLayer::new(opaque.clone())));
                let dark = if target.is_some() { (1.0 - tone.clamp(0.0, 1.0)) as f32 } else { 0.0 };
                let hi = spec.clamp(0.0, 1.0) as f32;
                let alpha = 1.0 - (1.0 - dark) * (1.0 - hi);
                if alpha > 0.0 {
                    let [sr, sg, sb] = target.unwrap_or(Color::WHITE).to_rgb();
                    let w_shade = (1.0 - hi) * dark;
                    let col = Color::rgb(
                        ((w_shade * sr + hi) / alpha).clamp(0.0, 1.0),
                        ((w_shade * sg + hi) / alpha).clamp(0.0, 1.0),
                        ((w_shade * sb + hi) / alpha).clamp(0.0, 1.0),
                    );
                    let mut overlay = FillLayer::new(Paint::solid(col));
                    overlay.opacity = alpha.clamp(0.0, 1.0);
                    items.push(AppearanceItem::Fill(overlay));
                }
            }
            let items = items
                .into_iter()
                .flat_map(|item| {
                    let AppearanceItem::Fill(fill) = &item else { return vec![item] };
                    let mut edge = classic_seam(fill.paint.clone(), seam);
                    edge.opacity = fill.opacity;
                    vec![item, AppearanceItem::Stroke(edge)]
                })
                .collect();
            Arc::new(Node::path(n.id, face.path(), Appearance { items, ..Default::default() }))
        })
        .collect();
    let mut group = Node::group(n.id, children);
    group.opacity = opacity;
    group.blend = blend;
    if let Some(mask) = mask {
        let bounds = group.visual_bounds().unwrap_or(Rect::ZERO).inflate(1.0, 1.0);
        let coverage = Node::path(n.id, vectorcraft_geom::shapes::rectangle(bounds), Appearance::basic(mask, Paint::None, 0.0));
        group.extras_mut().mask = Some(Box::new(OpacityMask::new(coverage, true)));
    }
    group
}
