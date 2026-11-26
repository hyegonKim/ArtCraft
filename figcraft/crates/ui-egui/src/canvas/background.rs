//! Painting the canvas background around (or behind) the presented render texture.

use egui::{Color32, CornerRadius, Painter, Rect, pos2};

/// Paint `bg` across `rect`. When `image_rect` covers part of `rect` and `bg` is opaque, fills
/// `rect` with `bg` so uncovered margins around a panned or zoomed texture show the page colour
/// cleanly without flicker.
pub(super) fn paint(painter: &Painter, rect: Rect, bg: Color32, image_rect: Option<Rect>) {
    let Some(ir) = image_rect.map(|r| r.intersect(rect)).filter(|r| r.is_positive()) else {
        painter.rect_filled(rect, CornerRadius::ZERO, bg);
        return;
    };
    if ir == rect {
        painter.rect_filled(rect, CornerRadius::ZERO, bg);
        return;
    }
    // Paint background over the full canvas rect (the texture draws over `image_rect` right after,
    // including any translucent pixels when `image_rect` has transparency) and ensure crisp margin
    // strips around `ir` have no seam.
    painter.rect_filled(rect, CornerRadius::ZERO, bg);
    let strips = [
        Rect::from_min_max(rect.min, pos2(rect.max.x, ir.min.y)),
        Rect::from_min_max(pos2(rect.min.x, ir.max.y), rect.max),
        Rect::from_min_max(pos2(rect.min.x, ir.min.y), pos2(ir.min.x, ir.max.y)),
        Rect::from_min_max(pos2(ir.max.x, ir.min.y), pos2(rect.max.x, ir.max.y)),
    ];
    for s in strips {
        if s.is_positive() {
            painter.rect_filled(s, CornerRadius::ZERO, bg);
        }
    }
}
