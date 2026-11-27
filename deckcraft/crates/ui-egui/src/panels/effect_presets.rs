use deckcraft_model::Effects;
use egui::Ui;

pub(super) const SHADOW_PRESETS: &[(&str, &str)] =
    &[("None", "none"), ("Outer", "outer"), ("Bottom", "bottom"), ("Center", "center"), ("Inner", "inner"), ("Perspective", "perspective")];

pub(super) const GLOW_PRESETS: &[(&str, f64)] = &[("No Glow", 0.0), ("5 pt", 5.0), ("8 pt", 8.0), ("11 pt", 11.0), ("18 pt", 18.0)];

pub(super) const SOFT_EDGE_PRESETS: &[(&str, f64)] =
    &[("No Soft Edges", 0.0), ("1 pt", 1.0), ("2.5 pt", 2.5), ("5 pt", 5.0), ("10 pt", 10.0), ("25 pt", 25.0)];

/// Identifies which shadow preset `fx` currently matches, or `None` when modified by sliders.
pub(super) fn shadow_preset_id(fx: &Effects) -> Option<&'static str> {
    if let Some(s) = &fx.inner_shadow {
        if (s.blur - 4.0).abs() < 0.05 && (s.dist - 3.0).abs() < 0.05 && (s.dir - 45.0).abs() < 0.5 {
            return Some("inner");
        }
        return None;
    }
    let Some(s) = &fx.outer_shadow else {
        return Some("none");
    };
    if (s.sy - 0.23).abs() < 0.05 && s.kx.abs() > 1.0 {
        Some("perspective")
    } else if s.dist.abs() < 0.05 && (s.blur - 8.0).abs() < 0.05 {
        Some("center")
    } else if (s.dir - 90.0).abs() < 0.5 && (s.dist - 3.0).abs() < 0.05 && (s.blur - 4.0).abs() < 0.05 {
        Some("bottom")
    } else if (s.dir - 45.0).abs() < 0.5 && (s.dist - 3.0).abs() < 0.05 && (s.blur - 4.0).abs() < 0.05 {
        Some("outer")
    } else {
        None
    }
}

pub(super) fn shadow_row(ui: &mut Ui, active: Option<&str>) -> Option<&'static str> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for &(label, id) in SHADOW_PRESETS {
            if ui.add(egui::Button::new(label).selected(active == Some(id))).clicked() {
                picked = Some(id);
            }
        }
    });
    picked
}

pub(super) fn glow_row(ui: &mut Ui, radius: f64) -> Option<f64> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for &(label, r) in GLOW_PRESETS {
            let selected = if r == 0.0 { radius <= 0.0 } else { (radius - r).abs() < 0.05 };
            if ui.add(egui::Button::new(label).selected(selected)).clicked() {
                picked = Some(r);
            }
        }
    });
    picked
}

pub(super) fn soft_edge_row(ui: &mut Ui, radius: f64) -> Option<f64> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for &(label, r) in SOFT_EDGE_PRESETS {
            let selected = if r == 0.0 { radius <= 0.0 } else { (radius - r).abs() < 0.05 };
            if ui.add(egui::Button::new(label).selected(selected)).clicked() {
                picked = Some(r);
            }
        }
    });
    picked
}
