//! Raster effects stay where they are, and look the same, at every zoom and pan (#755).

use serde_json::json;
use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_doc::text::{CharStyle, TextObject};
use vectorcraft_doc::{Appearance, Effect, Node, NodeKind};
use vectorcraft_geom::{Point, shapes};

use super::*;

/// Type blurred by `radius` points (Gaussian Blur, or Feather when `feather`) over a rectangle
/// that overlays at half opacity; without the type when `radius` is `None`.
fn scene(radius: Option<f64>, feather: bool) -> Document {
    let mut d = Document::new(200.0, 100.0);
    let l = d.layers[0].id;
    let mut back = Node::path(
        d.alloc_id(),
        shapes::rectangle(Rect::new(0.0, 0.0, 200.0, 100.0)),
        Appearance::basic(Paint::solid(Color::rgb(0.2, 0.4, 0.9)), Paint::None, 0.0),
    );
    back.blend = BlendMode::Overlay;
    back.opacity = 0.5;
    d.insert(Some(l), 0, back).unwrap();
    if let Some(radius) = radius {
        let style = CharStyle { size: 36.0, ..CharStyle::default() };
        let mut t = Node::new(d.alloc_id(), NodeKind::Text(Box::new(TextObject::point(Point::new(30.0, 60.0), "Hello", style))));
        let id = if feather { "stylize.feather" } else { "blur.gaussian" };
        t.appearance.effects.push(Effect { id: id.into(), params: json!({"radius": radius}), visible: true });
        d.insert(Some(l), 1, t).unwrap();
    }
    d
}

/// `d` with its layer's art in one group: the overlay and the type grouped, as in #755's report.
fn grouped(mut d: Document) -> Document {
    let l = d.layers[0].id;
    let id = d.alloc_id();
    let members = d.children_mut(Some(l)).map(std::mem::take).unwrap();
    d.insert(Some(l), 0, Node::new(id, NodeKind::Group { children: members, clip: false })).unwrap();
    d
}

/// The canvas's options: art on nothing, no artboards.
fn screen() -> RenderOptions {
    RenderOptions { background: None, artboards: false, ..Default::default() }
}

fn render(d: &Document, threads: u16, view: Affine, (w, h): (u32, u32)) -> Rendered {
    let mut r = Renderer::new();
    r.threads = threads;
    r.render(d, w, h, view, &screen())
}

/// Where the type's ink is on the document (its centroid), rendered `threads`-threaded through
/// `view` on a `w` × `h` canvas.
fn ink_centre(radius: f64, threads: u16, view: Affine, size: (u32, u32)) -> Point {
    let with = render(&scene(Some(radius), false), threads, view, size);
    let without = render(&scene(None, false), threads, view, size);
    let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
    for (i, (p, q)) in with.pixels.chunks(4).zip(without.pixels.chunks(4)).enumerate() {
        let d: f64 = (0..4).map(|c| f64::from(p[c].abs_diff(q[c]))).sum();
        let (x, y) = ((i % size.0 as usize) as f64 + 0.5, (i / size.0 as usize) as f64 + 0.5);
        sx += d * x;
        sy += d * y;
        sw += d;
    }
    assert!(sw > 0.0, "the type shows");
    view.inverse() * Point::new(sx / sw, sy / sw)
}

#[test]
fn blurred_type_stays_in_place_at_every_zoom_and_pan() {
    for (radius, threads) in [(3.0, 0), (3.0, 4), (12.0, 0), (12.0, 4)] {
        let base = ink_centre(radius, threads, Affine::IDENTITY, (200, 100));
        // The whole document in view, at other zooms and moved by parts of a pixel.
        for (zoom, pan) in
            [(1.0, (13.3, 7.6)), (2.0, (41.7, 20.2)), (3.3, (15.25, 9.5)), (0.5, (10.4, 5.1)), (6.0, (30.6, 25.3)), (1.37, (0.5, 0.25))]
        {
            let view = Affine::translate(pan) * Affine::scale(zoom);
            let size = ((200.0 * zoom + 2.0 * pan.0).ceil() as u32, (100.0 * zoom + 2.0 * pan.1).ceil() as u32);
            let at = ink_centre(radius, threads, view, size);
            assert!(
                (at.x - base.x).abs() < 1.0 && (at.y - base.y).abs() < 1.0,
                "radius {radius}, threads {threads}, zoom {zoom}, pan {pan:?}: {at:?} vs {base:?}"
            );
        }
    }
}

/// Scrolling moves the blurred type with the page and nothing else: two views a whole number of
/// pixels apart draw the same pixels where they overlap, whether the type fits on the canvas or
/// runs off it.
#[test]
fn scrolling_moves_blurred_type_without_changing_it() {
    for (feather, group) in [(false, false), (true, false), (false, true)] {
        let d = scene(Some(12.0), feather);
        let d = if group { grouped(d) } else { d };
        for threads in [0, 4] {
            for (zoom, (w, h)) in [(1.0, (240, 140)), (4.0, (500, 300)), (8.0, (400, 300))] {
                let at = |dx: f64, dy: f64| render(&d, threads, Affine::translate((-dx, -dy)) * Affine::scale(zoom), (w, h));
                let a = at(20.0, 10.0);
                for (sx, sy) in [(1u32, 0u32), (3, 2), (37, 23), (101, 57)] {
                    let b = at(20.0 + f64::from(sx), 10.0 + f64::from(sy));
                    let mut worst = 0;
                    for y in sy..h {
                        for x in sx..w {
                            let i = ((y * w + x) * 4) as usize;
                            let j = (((y - sy) * w + x - sx) * 4) as usize;
                            for c in 0..4 {
                                worst = worst.max(a.pixels[i + c].abs_diff(b.pixels[j + c]));
                            }
                        }
                    }
                    assert!(
                        worst <= 3,
                        "feather {feather}, grouped {group}, threads {threads}, zoom {zoom}, scrolled by ({sx}, {sy}): a pixel changed by {worst}"
                    );
                }
            }
        }
    }
}

/// Scrolling by parts of a pixel (a trackpad, a zoom about the pointer) moves heavily blurred type
/// by just that: its ink stays put on the page within a tenth of a pixel, at any zoom.
#[test]
fn blurred_type_doesnt_wobble_as_the_view_moves_by_parts_of_a_pixel() {
    for threads in [0, 4] {
        for zoom in [1.0, 4.0] {
            let at = |p: f64| {
                ink_centre(30.0, threads, Affine::translate((400.0 - 75.0 * zoom + p, 330.0 - 48.0 * zoom + p)) * Affine::scale(zoom), (800, 660))
            };
            let base = at(0.0);
            for k in 1..10 {
                let c = at(f64::from(k) * 0.1);
                // How far it moved on the screen, besides the view's own move.
                let (dx, dy) = ((c.x - base.x) * zoom, (c.y - base.y) * zoom);
                assert!(dx.abs() < 0.1 && dy.abs() < 0.1, "threads {threads}, zoom {zoom}, moved by {}: off by ({dx}, {dy}) px", f64::from(k) * 0.1);
            }
        }
    }
}

/// Covariance of one blurred dot follows the full canvas affine, including its zero-width axis.
#[test]
fn anisotropic_blur_follows_rotated_reflected_and_nonuniform_views() {
    fn moments(image: &Rendered, width: usize) -> (Point, [f64; 3]) {
        let mut mass = 0.0;
        let mut sum = [0.0; 5];
        for (index, pixel) in image.pixels.as_chunks::<4>().0.iter().enumerate() {
            let weight = f64::from(pixel[3]);
            let (x, y) = ((index % width) as f64 + 0.5, (index / width) as f64 + 0.5);
            mass += weight;
            for (value, sample) in sum.iter_mut().zip([x, y, x * x, x * y, y * y]) {
                *value += sample * weight;
            }
        }
        assert!(mass > 0.0);
        let (x, y) = (sum[0] / mass, sum[1] / mass);
        (Point::new(x, y), [sum[2] / mass - x * x, sum[3] / mass - x * y, sum[4] / mass - y * y])
    }
    let mut document = Document::new(140.0, 140.0);
    let layer = document.layers[0].id;
    let mut dot = Node::path(
        document.alloc_id(),
        shapes::rectangle(Rect::new(69.0, 69.0, 71.0, 71.0)),
        Appearance::basic(Paint::solid(Color::WHITE), Paint::None, 0.0),
    );
    dot.appearance.effects.push(Effect {
        id: "blur.gaussian".into(),
        params: json!({"radius": 12.0, "anisotropyX": 1.0, "anisotropyY": 0.0}),
        visible: true,
    });
    document.insert(Some(layer), 0, dot).unwrap();
    let base_trace = super::fx::anisotropic_trace::begin("base");
    let base = render(&document, 1, Affine::IDENTITY, (140, 140));
    super::fx::anisotropic_trace::output("base.final", Affine::IDENTITY, &base);
    drop(base_trace);
    let (center, covariance) = moments(&base, 140);
    assert!(covariance[0] > 20.0 && covariance[2] < 1.0, "{covariance:?}");
    // Reuse a renderer across view changes as the real canvas does. A fresh renderer alone
    // cannot detect an incorrectly reused view-dependent filtered raster.
    let mut cached = Renderer::new();
    cached.threads = 1;
    let _ = cached.render(&document, 140, 140, Affine::IDENTITY, &screen());
    for transform in [
        Affine::rotate(std::f64::consts::FRAC_PI_2),
        Affine::rotate(0.53),
        Affine::scale_non_uniform(-1.0, 1.0),
        Affine::rotate(0.53) * Affine::scale_non_uniform(1.5, 0.75),
    ] {
        let view = Affine::translate((100.0, 100.0)) * transform * Affine::translate((-center.x, -center.y));
        let _view_trace = super::fx::anisotropic_trace::begin("view");
        let actual = render(&document, 1, view, (200, 200));
        super::fx::anisotropic_trace::output("view.final", view, &actual);
        let (position, got) = moments(&actual, 200);
        assert!((position.x - 100.0).abs() < 1.0 && (position.y - 100.0).abs() < 1.0, "{position:?}");
        let [a, b, c, d, _, _] = transform.as_coeffs();
        let [xx, xy, yy] = covariance;
        let expected =
            [a * a * xx + 2.0 * a * c * xy + c * c * yy, a * b * xx + (a * d + b * c) * xy + c * d * yy, b * b * xx + 2.0 * b * d * xy + d * d * yy];
        // The medium-quality final affine resampling may add less than a pixel of spread.
        let tolerance = 1.0 + 0.08 * expected[0].max(expected[2]);
        for (actual, expected) in got.into_iter().zip(expected) {
            assert!((actual - expected).abs() < tolerance, "view={view:?}, got={got:?}, expected={expected:?}");
        }
        assert_eq!(render(&document, 1, view, (200, 200)).pixels, actual.pixels);
        for _ in 0..2 {
            assert_eq!(
                cached.render(&document, 200, 200, view, &screen()).pixels,
                actual.pixels,
                "a reused canvas renderer must match a fresh one after {view:?}"
            );
        }
    }
}

/// Fractional viewport pans preserve the document's blur axes and its cached raster contract.
#[test]
fn anisotropic_blur_fractional_pan_keeps_covariance_and_fresh_cached_pixels() {
    fn moments(image: &Rendered) -> (Point, [f64; 3]) {
        let mut mass = 0.0;
        let mut sum = [0.0; 5];
        for (index, pixel) in image.pixels.as_chunks::<4>().0.iter().enumerate() {
            let weight = f64::from(pixel[3]);
            let (x, y) = ((index % image.width as usize) as f64 + 0.5, (index / image.width as usize) as f64 + 0.5);
            mass += weight;
            for (value, sample) in sum.iter_mut().zip([x, y, x * x, x * y, y * y]) {
                *value += sample * weight;
            }
        }
        assert!(mass > 0.0);
        let (x, y) = (sum[0] / mass, sum[1] / mass);
        (Point::new(x, y), [sum[2] / mass - x * x, sum[3] / mass - x * y, sum[4] / mass - y * y])
    }
    let mut document = Document::new(140.0, 140.0);
    let layer = document.layers[0].id;
    let mut dot = Node::path(
        document.alloc_id(),
        shapes::rectangle(Rect::new(69.0, 69.0, 71.0, 71.0)),
        Appearance::basic(Paint::solid(Color::WHITE), Paint::None, 0.0),
    );
    dot.appearance.effects.push(Effect {
        id: "blur.gaussian".into(),
        params: json!({"radius": 12.0, "anisotropyX": 1.0, "anisotropyY": 0.0}),
        visible: true,
    });
    document.insert(Some(layer), 0, dot).unwrap();
    let base = render(&document, 1, Affine::IDENTITY, (140, 140));
    let (center, [xx, xy, yy]) = moments(&base);
    let mut cached = Renderer::new();
    cached.threads = 1;
    let _ = cached.render(&document, 140, 140, Affine::IDENTITY, &screen());
    for transform in [
        Affine::rotate(std::f64::consts::FRAC_PI_2),
        Affine::rotate(0.53),
        Affine::scale_non_uniform(-1.0, 1.0),
        Affine::rotate(0.53) * Affine::scale_non_uniform(1.5, 0.75),
    ] {
        let [a, b, c, d, _, _] = transform.as_coeffs();
        let expected =
            [a * a * xx + 2.0 * a * c * xy + c * c * yy, a * b * xx + (a * d + b * c) * xy + c * d * yy, b * b * xx + 2.0 * b * d * xy + d * d * yy];
        // Keep the existing regression's covariance bound, including the sharp zero axis.
        let tolerance = 1.0 + 0.08 * expected[0].max(expected[2]);
        for (dx, dy) in [(0.13, -0.27), (-0.41, 0.36), (0.71, -0.64)] {
            let view = Affine::translate((100.0 + dx, 100.0 + dy)) * transform * Affine::translate((-center.x, -center.y));
            let fresh = render(&document, 1, view, (200, 200));
            let (position, got) = moments(&fresh);
            assert!((position.x - 100.0 - dx).abs() < 1.0 && (position.y - 100.0 - dy).abs() < 1.0, "{position:?}");
            for (actual, expected) in got.into_iter().zip(expected) {
                assert!((actual - expected).abs() < tolerance, "view={view:?}, got={got:?}, expected={expected:?}");
            }
            assert_eq!(render(&document, 1, view, (200, 200)).pixels, fresh.pixels);
            for _ in 0..2 {
                assert_eq!(cached.render(&document, 200, 200, view, &screen()).pixels, fresh.pixels, "fractional-pan cached canvas {view:?}");
            }
        }
    }
}

/// Observation only: compare the existing precise modes without changing the failing oracle.
/// Root collects println output with RUST_TEST_NOCAPTURE=1 and the existing opt-in stage trace.
#[test]
fn anisotropic_blur_precision_ab_diagnostic() {
    fn moments(image: &Rendered) -> (f64, Point, [f64; 3]) {
        let mut mass = 0.0;
        let mut sum = [0.0; 5];
        for (index, pixel) in image.pixels.as_chunks::<4>().0.iter().enumerate() {
            let weight = f64::from(pixel[3]);
            let (x, y) = ((index % image.width as usize) as f64 + 0.5, (index / image.width as usize) as f64 + 0.5);
            mass += weight;
            for (value, sample) in sum.iter_mut().zip([x, y, x * x, x * y, y * y]) {
                *value += sample * weight;
            }
        }
        assert!(mass > 0.0);
        let (x, y) = (sum[0] / mass, sum[1] / mass);
        (mass, Point::new(x, y), [sum[2] / mass - x * x, sum[3] / mass - x * y, sum[4] / mass - y * y])
    }

    let mut document = Document::new(140.0, 140.0);
    let layer = document.layers[0].id;
    let mut dot = Node::path(
        document.alloc_id(),
        shapes::rectangle(Rect::new(69.0, 69.0, 71.0, 71.0)),
        Appearance::basic(Paint::solid(Color::WHITE), Paint::None, 0.0),
    );
    dot.appearance.effects.push(Effect {
        id: "blur.gaussian".into(),
        params: json!({"radius": 12.0, "anisotropyX": 1.0, "anisotropyY": 0.0}),
        visible: true,
    });
    document.insert(Some(layer), 0, dot).unwrap();
    let options = screen();
    let transform = Affine::rotate(0.53) * Affine::scale_non_uniform(1.5, 0.75);
    let (mut reference_center, mut reference_covariance) = (Point::ZERO, [0.0; 3]);
    for (case, pan, size, labels) in [
        ("base", None, (140, 140), ("precision.base.false", "precision.base.true", "precision.base.high_fast")),
        ("nonuniform", Some((0.0, 0.0)), (200, 200), ("precision.nonuniform.false", "precision.nonuniform.true", "precision.nonuniform.high_fast")),
        ("fractional_pan", Some((0.13, -0.27)), (200, 200), ("precision.pan.false", "precision.pan.true", "precision.pan.high_fast")),
    ] {
        let view = match pan {
            Some((dx, dy)) => Affine::translate((100.0 + dx, 100.0 + dy)) * transform * Affine::translate((-reference_center.x, -reference_center.y)),
            None => Affine::IDENTITY,
        };
        let mut fast_pixels: Option<Vec<u8>> = None;
        for (mode, precise, high, label) in
            [("MediumFast", false, false, labels.0), ("MediumPrecise", true, false, labels.1), ("HighFast", false, true, labels.2)]
        {
            let opts = RenderOptions { precise, ..options.clone() };
            let sampler = if high { vello_cpu::peniko::ImageQuality::High } else { vello_cpu::peniko::ImageQuality::Medium };
            let sampler_override = super::fx::anisotropic_trace::override_final_sampler(Some(sampler));
            let trace = super::fx::anisotropic_trace::begin(label);
            let mut renderer = Renderer::new();
            renderer.threads = 1;
            let image = renderer.render(&document, size.0, size.1, view, &opts);
            super::fx::anisotropic_trace::output("precision_ab.final", view, &image);
            drop(trace);
            drop(sampler_override);
            assert_eq!((image.width, image.height), size);
            assert_eq!(image.pixels.len(), size.0 as usize * size.1 as usize * 4);
            let (mass, center, covariance) = moments(&image);
            if case == "base" && mode == "MediumFast" {
                reference_center = center;
                reference_covariance = covariance;
            }
            let [a, b, c, d, _, _] = view.as_coeffs();
            let [xx, xy, yy] = reference_covariance;
            let expected = [
                a * a * xx + 2.0 * a * c * xy + c * c * yy,
                a * b * xx + (a * d + b * c) * xy + c * d * yy,
                b * b * xx + 2.0 * b * d * xy + d * d * yy,
            ];
            let expected_center = view * reference_center;
            let tolerance = 1.0 + 0.08 * expected[0].max(expected[2]);
            let covariance_within_bound = covariance.into_iter().zip(expected).all(|(got, want)| (got - want).abs() < tolerance);
            let centroid_within_bound = (center.x - expected_center.x).abs() < 1.0 && (center.y - expected_center.y).abs() < 1.0;
            let images_equal = fast_pixels.as_ref().map(|pixels| pixels == &image.pixels);
            let different_pixels = fast_pixels
                .as_ref()
                .map(|pixels| pixels.as_chunks::<4>().0.iter().zip(image.pixels.as_chunks::<4>().0.iter()).filter(|(a, b)| a != b).count());
            println!(
                "VEC_PRECISION_AB {}",
                json!({ "case": case, "mode": mode, "precise": precise, "sampler": if high { "High" } else { "Medium" },
                    "high_final_override": high, "sampler_override": if high { "SomeHigh" } else { "SomeMedium" },
                    "production_default_sampler": "High", "reference_mode": "MediumFast", "view": view.as_coeffs(), "size": [image.width, image.height],
                    "mass": mass, "centroid": [center.x, center.y], "covariance": covariance,
                    "reference_covariance": reference_covariance, "expected_covariance": expected,
                    "expected_centroid": [expected_center.x, expected_center.y], "existing_covariance_tolerance": tolerance,
                    "centroid_within_existing_bound": centroid_within_bound, "covariance_within_existing_bound": covariance_within_bound,
                    "within_existing_view_oracle": centroid_within_bound && covariance_within_bound,
                    "images_equal_to_fast": images_equal, "different_pixels_from_fast": different_pixels,
                    "observation_only": true })
            );
            if mode == "MediumFast" {
                fast_pixels = Some(image.pixels);
            }
        }
    }
}
