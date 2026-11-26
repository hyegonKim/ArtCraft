use crate::graph::{GraphCamera, canvas_geometry::CanvasGeometry, frame_hit::FrameHit};
use egui::{Rect, Response};

/// Geometry and annotation hit targets produced by one FX-Map canvas frame.
pub(super) struct PatternCanvas {
    pub geometry: CanvasGeometry,
    pub response: Response,
    pub camera: GraphCamera,
    pub frames: Vec<FrameHit>,
    pub comments: Vec<(u64, Rect)>,
    pub pins: Vec<(u64, Rect)>,
}
