//! Map module (`ViewMode::Map`): interactive Web-Mercator GPS map of library photos, coordinate
//! pinning (`photo.setGps`), GPX tracklog tagging (`photo.tagFromTracklog`), and reverse geocoding.

use std::f64::consts::PI;

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use lightcraft_catalog::PhotoId;
use serde_json::json;

use crate::LightcraftApp;
use crate::state::ViewMode;
use crate::theme::Tokens;
use crate::widgets::{register, text_button};

/// Simplified clean-room continental coastline loops `(lat, lon)` for offline world context.
const CONTINENTS: &[&[(f64, f64)]] = &[
    // North America
    &[
        (70.0, -165.0),
        (71.0, -130.0),
        (60.0, -65.0),
        (45.0, -60.0),
        (25.0, -80.0),
        (18.0, -96.0),
        (8.0, -79.0),
        (16.0, -100.0),
        (32.0, -117.0),
        (48.0, -124.0),
        (60.0, -148.0),
        (70.0, -165.0),
    ],
    // South America
    &[(11.0, -75.0), (6.0, -52.0), (-6.0, -35.0), (-23.0, -43.0), (-54.0, -68.0), (-42.0, -74.0), (-18.0, -71.0), (-4.0, -81.0), (11.0, -75.0)],
    // Europe + Asia
    &[
        (36.0, -9.0),
        (43.0, -9.0),
        (48.0, -4.0),
        (58.0, 6.0),
        (70.0, 28.0),
        (72.0, 110.0),
        (66.0, 170.0),
        (52.0, 142.0),
        (35.0, 139.0),
        (35.0, 129.0),
        (22.0, 114.0),
        (9.0, 105.0),
        (2.0, 103.0),
        (16.0, 96.0),
        (22.0, 88.0),
        (8.0, 77.0),
        (24.0, 67.0),
        (25.0, 56.0),
        (13.0, 44.0),
        (30.0, 33.0),
        (37.0, 26.0),
        (41.0, 14.0),
        (36.0, -5.0),
        (36.0, -9.0),
    ],
    // Africa
    &[
        (35.0, -6.0),
        (37.0, 10.0),
        (31.0, 32.0),
        (12.0, 51.0),
        (-2.0, 41.0),
        (-26.0, 33.0),
        (-34.0, 18.0),
        (-18.0, 12.0),
        (4.0, 9.0),
        (5.0, -8.0),
        (15.0, -17.0),
        (35.0, -6.0),
    ],
    // Australia
    &[(-12.0, 131.0), (-11.0, 142.0), (-28.0, 153.0), (-38.0, 145.0), (-34.0, 115.0), (-22.0, 114.0), (-12.0, 131.0)],
];

fn mercator_norm(lat: f64, lon: f64) -> (f64, f64) {
    let clamped_lat = lat.clamp(-85.0, 85.0);
    let x = (lon + 180.0) / 360.0;
    let rad = clamped_lat.to_radians();
    let y = (1.0 - (rad.tan() + 1.0 / rad.cos()).ln() / PI) * 0.5;
    (x, y.clamp(0.0, 1.0))
}

fn inv_mercator_norm(nx: f64, ny: f64) -> (f64, f64) {
    let lon = (nx.rem_euclid(1.0)) * 360.0 - 180.0;
    let n = PI * (1.0 - 2.0 * ny.clamp(0.0, 1.0));
    let lat = n.sinh().atan().to_degrees().clamp(-85.0, 85.0);
    (lat, lon)
}

fn world_px(zoom: f32) -> f64 {
    256.0 * (2.0_f64).powf(zoom.clamp(1.0, 16.0) as f64)
}

fn project(lat: f64, lon: f64, center: (f64, f64), zoom: f32, rect: Rect) -> Pos2 {
    let wpx = world_px(zoom);
    let (cx, cy) = mercator_norm(center.0, center.1);
    let (px, py) = mercator_norm(lat, lon);
    let mut dx = px - cx;
    if dx > 0.5 {
        dx -= 1.0;
    } else if dx < -0.5 {
        dx += 1.0;
    }
    let dy = py - cy;
    pos2(rect.center().x + (dx * wpx) as f32, rect.center().y + (dy * wpx) as f32)
}

fn unproject(pos: Pos2, center: (f64, f64), zoom: f32, rect: Rect) -> (f64, f64) {
    let wpx = world_px(zoom);
    let (cx, cy) = mercator_norm(center.0, center.1);
    let nx = cx + ((pos.x - rect.center().x) as f64) / wpx;
    let ny = (cy + ((pos.y - rect.center().y) as f64) / wpx).clamp(0.0, 1.0);
    inv_mercator_norm(nx, ny)
}

pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let visible = app.session.visible_cloned();
    let mut tagged: Vec<(PhotoId, f64, f64, String, String)> = Vec::new();
    for id in &visible {
        if let Some(p) = app.session.catalog.photo(*id)
            && let Some((lat, lon)) = p.meta.gps
            && lat.is_finite()
            && lon.is_finite()
        {
            let place = match (p.meta.city.as_str(), p.meta.country.as_str()) {
                ("", "") => String::new(),
                (c, "") => c.to_string(),
                ("", co) => co.to_string(),
                (c, co) => format!("{c}, {co}"),
            };
            tagged.push((*id, lat, lon, p.file_name.clone(), place));
        }
    }

    egui::Panel::top("map_toolbar")
        .exact_size(38.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: 14, right: 14, top: 4, bottom: 4 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr("Map")).font(t.semibold(14.0)).color(t.text));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(crate::i18n::tr_format!("{tagged} of {total} geotagged", tagged = tagged.len(), total = visible.len()))
                        .color(t.text_dim)
                        .size(12.0),
                );
                ui.add_space(10.0);
                if text_button(ui, "mapZoomOut", "−", false).clicked() {
                    app.ui.map_zoom = (app.ui.map_zoom - 1.0).clamp(1.0, 16.0);
                }
                ui.label(egui::RichText::new(format!("{}×", app.ui.map_zoom.round() as i32)).color(t.text_label).size(11.5));
                if text_button(ui, "mapZoomIn", "+", false).clicked() {
                    app.ui.map_zoom = (app.ui.map_zoom + 1.0).clamp(1.0, 16.0);
                }
                if !tagged.is_empty() && text_button(ui, "mapFitPins", crate::i18n::tr("Fit Pins"), false).clicked() {
                    let sum_lat: f64 = tagged.iter().map(|p| p.1).sum();
                    let sum_lon: f64 = tagged.iter().map(|p| p.2).sum();
                    let n = tagged.len() as f64;
                    app.ui.map_center = (sum_lat / n, sum_lon / n);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let has_sel = !app.session.selected().is_empty();
                    if ui
                        .add_enabled(has_sel, egui::Button::new(crate::i18n::tr("Pin Selected Here")))
                        .on_hover_text(crate::i18n::tr("Assign the map centre coordinates (or Alt-click on the map) to the selected photos"))
                        .clicked()
                    {
                        let (lat, lon) = app.ui.map_center;
                        let ids: Vec<u64> = app.session.selected().iter().map(|p| p.0).collect();
                        let _ = app.run("photo.setGps", json!({"ids": ids, "lat": lat, "lon": lon}));
                    }
                    if ui.add_enabled(!tagged.is_empty(), egui::Button::new(crate::i18n::tr("Look Up Places"))).clicked() {
                        let ids: Vec<u64> = tagged.iter().map(|p| p.0.0).collect();
                        let _ = app.run("photo.geocode", json!({"ids": ids}));
                    }
                    if app.services.pick_tracklog.is_some() && has_sel && ui.button(crate::i18n::tr("Tag from GPX…")).clicked() {
                        let _ = app.run("photo.tagFromTracklog", json!({}));
                    }
                });
            });
        });

    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    register(ui, "map:canvas", rect);

    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_rgb(19, 26, 36));

    // Wheel zoom + drag pan
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.5 {
            app.ui.map_zoom = (app.ui.map_zoom + scroll * 0.015).clamp(1.0, 16.0);
        }
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        let wpx = world_px(app.ui.map_zoom);
        let (cx, cy) = mercator_norm(app.ui.map_center.0, app.ui.map_center.1);
        let nx = cx - (d.x as f64) / wpx;
        let ny = (cy - (d.y as f64) / wpx).clamp(0.0, 1.0);
        app.ui.map_center = inv_mercator_norm(nx, ny);
    }

    // Graticule lines
    let grid_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 150, 180, 28));
    for lat in [-60.0, -30.0, 0.0, 30.0, 60.0] {
        let y = project(lat, app.ui.map_center.1, app.ui.map_center, app.ui.map_zoom, rect).y;
        if (rect.top()..=rect.bottom()).contains(&y) {
            p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], grid_stroke);
        }
    }
    for lon in [-120.0, -60.0, 0.0, 60.0, 120.0] {
        let x = project(0.0, lon, app.ui.map_center, app.ui.map_zoom, rect).x;
        if (rect.left()..=rect.right()).contains(&x) {
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], grid_stroke);
        }
    }

    // Continental outlines
    let land_stroke = Stroke::new(1.2, Color32::from_rgb(58, 76, 96));
    let land_fill = Color32::from_rgba_unmultiplied(42, 56, 70, 130);
    for poly in CONTINENTS {
        let pts: Vec<Pos2> = poly.iter().map(|&(lat, lon)| project(lat, lon, app.ui.map_center, app.ui.map_zoom, rect)).collect();
        if pts.len() >= 3 {
            // Skip wraparound jumps across the antimeridian
            let max_step = pts.windows(2).map(|w| (w[1].x - w[0].x).abs()).fold(0.0f32, f32::max);
            if max_step < rect.width() * 0.75 {
                p.add(egui::Shape::convex_polygon(pts.clone(), land_fill, Stroke::NONE));
                p.add(egui::Shape::line(pts, land_stroke));
            }
        }
    }

    // Center crosshair
    let c = rect.center();
    let cross = Stroke::new(1.0, Color32::from_white_alpha(65));
    p.line_segment([pos2(c.x - 6.0, c.y), pos2(c.x + 6.0, c.y)], cross);
    p.line_segment([pos2(c.x, c.y - 6.0), pos2(c.x, c.y + 6.0)], cross);

    // Cluster nearby pins within 22 px so dense shoots stay legible
    struct Cluster {
        pos: Pos2,
        ids: Vec<PhotoId>,
        lat: f64,
        lon: f64,
        sample_name: String,
        place: String,
    }
    let mut clusters: Vec<Cluster> = Vec::new();
    for (id, lat, lon, name, place) in &tagged {
        let pt = project(*lat, *lon, app.ui.map_center, app.ui.map_zoom, rect);
        if !rect.expand(24.0).contains(pt) {
            continue;
        }
        if let Some(cl) = clusters.iter_mut().find(|cl| cl.pos.distance(pt) < 22.0) {
            cl.ids.push(*id);
        } else {
            clusters.push(Cluster { pos: pt, ids: vec![*id], lat: *lat, lon: *lon, sample_name: name.clone(), place: place.clone() });
        }
    }

    let active = app.session.active();
    let mut clicked_cluster = false;
    for (idx, cl) in clusters.iter().enumerate() {
        let is_active = active.is_some_and(|a| cl.ids.contains(&a));
        let radius = if cl.ids.len() > 1 { 12.0 } else { 9.0 };
        let fill = if is_active { t.accent } else { Color32::from_rgb(225, 142, 48) };
        p.circle_filled(cl.pos, radius, fill);
        p.circle_stroke(cl.pos, radius, Stroke::new(1.5, Color32::WHITE));
        if cl.ids.len() > 1 {
            p.text(cl.pos, Align2::CENTER_CENTER, cl.ids.len().to_string(), t.semibold(10.5), Color32::BLACK);
        }
        let pin_rect = Rect::from_center_size(cl.pos, vec2(radius * 2.2, radius * 2.2));
        let pin_resp = ui.interact(pin_rect, egui::Id::unique(("map-pin", idx)), Sense::click());
        register(ui, format!("mapPin:{idx}"), pin_rect);
        let tip = if cl.place.is_empty() {
            format!("{} ({:.4}, {:.4})", cl.sample_name, cl.lat, cl.lon)
        } else {
            format!("{} · {} ({:.4}, {:.4})", cl.sample_name, cl.place, cl.lat, cl.lon)
        };
        let pin_resp = pin_resp.on_hover_text(tip);
        if pin_resp.double_clicked() {
            clicked_cluster = true;
            let ids: Vec<u64> = cl.ids.iter().map(|i| i.0).collect();
            let _ = app.run("library.select", json!({"ids": ids}));
            app.ui.view = ViewMode::Detail;
        } else if pin_resp.clicked() {
            clicked_cluster = true;
            let ids: Vec<u64> = cl.ids.iter().map(|i| i.0).collect();
            let _ = app.run("library.select", json!({"ids": ids}));
        }
    }

    // Alt-click on empty map assigns GPS coordinates to selected photos; double-click centres
    if !clicked_cluster && let Some(pos) = resp.interact_pointer_pos() {
        if resp.clicked() && ui.input(|i| i.modifiers.alt) && !app.session.selected().is_empty() {
            let (lat, lon) = unproject(pos, app.ui.map_center, app.ui.map_zoom, rect);
            let ids: Vec<u64> = app.session.selected().iter().map(|p| p.0).collect();
            let _ = app.run("photo.setGps", json!({"ids": ids, "lat": lat, "lon": lon}));
        } else if resp.double_clicked() {
            app.ui.map_center = unproject(pos, app.ui.map_center, app.ui.map_zoom, rect);
            app.ui.map_zoom = (app.ui.map_zoom + 1.0).clamp(1.0, 16.0);
        }
    }

    // Coordinate readout pill in bottom-left
    let (clat, clon) = app.ui.map_center;
    let coord_txt = format!("{clat:+.4}°, {clon:+.4}°");
    p.text(pos2(rect.left() + 12.0, rect.bottom() - 10.0), Align2::LEFT_BOTTOM, coord_txt, t.font(11.5), Color32::from_white_alpha(180));
}
