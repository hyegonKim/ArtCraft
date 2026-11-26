//! Animated slide transition & intra-slide object build frame sequence renderer.
//!
//! Steps through each exported slide's incoming transition (`Fade`, `Push`, `Wipe`, `Split`,
//! `Uncover`, `Cover`, `Zoom`, `Dissolve`, `Morph`, `Cut`, and all other transitions in
//! [`deckcraft_anim`]), then steps through its [`deckcraft_anim::Timeline`] object/paragraph build
//! animations at `fps`, followed by the slide's hold duration. Static hold intervals are emitted as
//! a single [`SequenceFrame`] with `ticks > 1` (each tick is `1 / fps` seconds) so animated GIF and
//! video encoders only render static intervals once.

use std::collections::HashMap;

use deckcraft_anim::{AnimState, Timeline, transition_frame, uses_slide_parts};
use deckcraft_color::Rgba;
use deckcraft_model::resolve::{self, Ctx};
use deckcraft_model::{Presentation, ShapeId};
use kurbo::Vec2;

use crate::{
    Image, MAX_SIDE, ParaState, PathMorph, RenderOpts, ShapeState, SlideParts, TextMorph, composite_transition, render_blend, render_slide,
};

/// Configuration for rendering an animated presentation frame sequence.
#[derive(Clone, Debug)]
pub struct SequenceOptions {
    /// Target frame width in pixels (`64..=4096`).
    pub width: u32,
    /// Optional explicit frame height in pixels; `None` derives height from `pres.slide_size`.
    pub height: Option<u32>,
    /// Round output width and height to multiples of 2 (required for 4:2:0 YUV video codecs).
    pub even_dimensions: bool,
    /// Frames per second (`1..=60`).
    pub fps: u32,
    /// Default hold time on each slide after transitions and builds finish, in seconds.
    pub hold_secs: f64,
    /// Pause between `OnClick` intra-slide build steps, in seconds.
    pub step_pause_secs: f64,
    /// Respect per-slide `Transition::advance_ms` when present.
    pub use_slide_timings: bool,
    /// Include slides marked `hidden`.
    pub include_hidden: bool,
    /// Explicit 0-based slide indices to render (`None` = all visible slides, or all if `include_hidden`).
    pub slides: Option<Vec<usize>>,
    /// Render intra-slide object & paragraph build animations (`Timeline`).
    pub include_builds: bool,
    /// Render inter-slide transitions (`Slide::transition`).
    pub include_transitions: bool,
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: None,
            even_dimensions: true,
            fps: 24,
            hold_secs: 2.0,
            step_pause_secs: 0.35,
            use_slide_timings: true,
            include_hidden: false,
            slides: None,
            include_builds: true,
            include_transitions: true,
        }
    }
}

/// One rendered RGBA frame (or coalesced run of `ticks` identical frames at `1 / fps` per tick).
#[derive(Clone, Debug)]
pub struct SequenceFrame {
    /// Premultiplied RGBA pixels (`width × height × 4`), matching [`Image`].
    pub image: Image,
    /// Number of `1 / fps` frame ticks this image remains on screen (`>= 1`).
    pub ticks: u32,
    /// 0-based slide index currently shown or arriving.
    pub slide: usize,
}

impl SequenceFrame {
    /// Duration of this frame in milliseconds at `fps`.
    pub fn duration_ms(&self, fps: u32) -> u32 {
        let fps = fps.max(1) as u64;
        ((u64::from(self.ticks.max(1)) * 1000 + fps / 2) / fps).clamp(1, u64::from(u32::MAX)) as u32
    }
}

/// Rendered sequence of frames with resolved dimensions and frame-rate metadata.
#[derive(Clone, Debug)]
pub struct RenderedSequence {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: Vec<SequenceFrame>,
}

impl RenderedSequence {
    /// Total number of `1 / fps` ticks across the entire sequence.
    pub fn total_ticks(&self) -> u64 {
        self.frames.iter().map(|f| u64::from(f.ticks.max(1))).sum()
    }

    /// Total duration of the sequence in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        let fps = u64::from(self.fps.max(1));
        (self.total_ticks() * 1000 + fps / 2) / fps
    }
}

/// Resolve output `(width, height)` in pixels for `pres` and `opts`.
pub fn sequence_dimensions(pres: &Presentation, opts: &SequenceOptions) -> (u32, u32) {
    let max = u32::from(MAX_SIDE).min(4096);
    let mut w = opts.width.clamp(64, max);
    let sw = pres.slide_size.width.max(1.0);
    let sh = pres.slide_size.height.max(1.0);
    let mut h = match opts.height {
        Some(eh) => eh.clamp(64, max),
        None => ((f64::from(w) * sh / sw).round() as u32).clamp(36, max),
    };
    if opts.even_dimensions {
        w = (w + (w & 1)).min(max & !1).max(64);
        h = (h + (h & 1)).min(max & !1).max(36);
    }
    (w, h)
}

fn anim_to_shape_state(tl: &Timeline, id: ShapeId, step: usize, t: f64) -> ShapeState {
    let a: AnimState = tl.state(id, None, step, t);
    let paras: Vec<(usize, ParaState)> = tl
        .paragraph_targets(id)
        .into_iter()
        .map(|pi| {
            let pa = tl.state(id, Some(pi), step, t);
            (
                pi,
                ParaState {
                    visible: pa.visible,
                    opacity: pa.opacity,
                    offset: Vec2::new(pa.offset_x, pa.offset_y),
                },
            )
        })
        .collect();
    ShapeState {
        visible: a.visible,
        opacity: a.opacity,
        offset: Vec2::new(a.offset_x, a.offset_y),
        scale: (a.scale_x, a.scale_y),
        rotate: a.rotate,
        clip: a.clip,
        tint: a.tint,
        paras,
    }
}

/// Build a [`Timeline`] for `slide_index` in `pres`, resolving master/layout placeholder transforms
/// and theme colours.
pub fn slide_timeline(pres: &Presentation, slide_index: usize) -> Timeline {
    let Some(slide) = pres.slides.get(slide_index) else { return Timeline::default() };
    let rctx = Ctx::for_slide(pres, slide);
    let scheme = rctx.as_ref().map(|c| &c.master.theme.colors);
    let mut xfrms: HashMap<ShapeId, deckcraft_geom::Xfrm> = HashMap::new();
    if let Some(c) = &rctx {
        deckcraft_model::walk(&slide.shapes, &mut |sh, _| {
            xfrms.entry(sh.id).or_insert_with(|| resolve::xfrm(c, sh));
        });
    }
    Timeline::with(slide, pres.slide_size.width, pres.slide_size.height, scheme, &|id| xfrms.get(&id).copied())
}

/// Render slide `index` at timeline position `(step, t)` (`step == tl.steps()` means all builds done;
/// `(0, 0.0)` means initial entrance state before any build runs).
pub fn render_slide_at_timeline(
    pres: &Presentation,
    index: usize,
    tl: &Timeline,
    step: usize,
    t: f64,
    base: &RenderOpts<'_>,
) -> Image {
    if tl.is_empty() {
        return render_slide(pres, index, base);
    }
    let mut map: HashMap<ShapeId, ShapeState> = HashMap::new();
    for id in tl.animated_shapes() {
        map.insert(id, anim_to_shape_state(tl, id, step, t));
    }
    let state_fn = |id: ShapeId| map.get(&id).cloned().unwrap_or_default();
    let opts = RenderOpts {
        scale: base.scale,
        edit: base.edit,
        show_hidden: base.show_hidden,
        state: Some(&state_fn),
        skip: base.skip,
        skip_text: base.skip_text,
        editing_text: base.editing_text,
        gif_times: base.gif_times,
        threads: base.threads,
        offset: base.offset,
        size: base.size,
        clear: base.clear,
        date_text: base.date_text.clone(),
        now: base.now,
        transparent_background: base.transparent_background,
    };
    render_slide(pres, index, &opts)
}

/// Render a transition frame at `t` (`0.0..=1.0`) from `prev` (`None` = black start of show) to
/// `index`, respecting any skipped hidden slides in between and supporting `Morph` as well as all
/// layer/part transitions (`Fade`, `Push`, `Wipe`, `Split`, `Uncover`, `Cover`, `Zoom`, `Dissolve`,
/// `Cut`, etc.).
pub fn render_transition_between(
    pres: &Presentation,
    prev: Option<usize>,
    index: usize,
    t: f64,
    opts: &RenderOpts<'_>,
) -> Image {
    let Some(slide) = pres.slides.get(index) else { return Image::default() };
    let (kind, option) = slide.transition.as_ref().map(|tr| (tr.kind.as_str(), tr.option.as_str())).unwrap_or(("none", ""));
    let tt = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    if kind == "morph"
        && let Some(from) = prev
        && let Some(a) = pres.slides.get(from)
        && tt < 1.0
    {
        let m = deckcraft_anim::morph_frame(pres, a, slide, tt, &|_| false);
        let f = |id: ShapeId| ShapeState { opacity: m.opacity_of(id), ..Default::default() };
        let o = RenderOpts {
            scale: opts.scale,
            edit: opts.edit,
            show_hidden: opts.show_hidden,
            state: Some(&f),
            skip: opts.skip,
            skip_text: opts.skip_text,
            editing_text: opts.editing_text,
            gif_times: opts.gif_times,
            threads: opts.threads,
            offset: opts.offset,
            size: opts.size,
            clear: opts.clear,
            date_text: opts.date_text.clone(),
            now: opts.now,
            transparent_background: opts.transparent_background,
        };
        let text: Vec<TextMorph> =
            m.text.iter().map(|x| TextMorph { old: x.old, new: x.new, from: x.from, to: x.to, chars: x.chars, t: m.mix }).collect();
        let paths: Vec<PathMorph> =
            m.paths.iter().map(|x| PathMorph { id: x.id, from: x.from.clone(), from_box: x.from_box, to_box: x.to_box, t: m.mix }).collect();
        return render_blend(pres, (from, index), m.mix, &m.shapes, &text, &paths, &o);
    }
    let tl = slide_timeline(pres, index);
    let new = render_slide_at_timeline(pres, index, &tl, 0, 0.0, opts);
    let old = prev.map(|i| {
        let old_tl = slide_timeline(pres, i);
        render_slide_at_timeline(pres, i, &old_tl, old_tl.steps(), 0.0, opts)
    });
    let parts = uses_slide_parts(kind, option).then(|| SlideParts::render(pres, prev, index, opts));
    let aspect = pres.slide_size.width.max(1.0) / pres.slide_size.height.max(1.0);
    composite_transition(old.as_ref(), &new, parts.as_ref(), &transition_frame(kind, option, tt, aspect))
}

fn push_coalesced(out: &mut Vec<SequenceFrame>, image: Image, ticks: u32, slide: usize) {
    let ticks = ticks.max(1);
    if let Some(last) = out.last_mut()
        && last.slide == slide
        && last.image.width == image.width
        && last.image.height == image.height
        && last.image.pixels == image.pixels
    {
        last.ticks = last.ticks.saturating_add(ticks);
        return;
    }
    out.push(SequenceFrame { image, ticks, slide });
}

/// Render the presentation's slide transitions, intra-slide builds, and slide holds into a timed
/// [`RenderedSequence`].
pub fn render_sequence(pres: &Presentation, opts: &SequenceOptions) -> RenderedSequence {
    let fps = opts.fps.clamp(1, 60);
    let dt = 1.0 / f64::from(fps);
    let (w, h) = sequence_dimensions(pres, opts);
    let slides: Vec<usize> = match &opts.slides {
        Some(list) => list
            .iter()
            .copied()
            .filter(|&i| i < pres.slides.len() && (opts.include_hidden || opts.slides.is_some() || !pres.slides[i].hidden))
            .collect(),
        None => (0..pres.slides.len()).filter(|&i| opts.include_hidden || !pres.slides[i].hidden).collect(),
    };
    if slides.is_empty() {
        return RenderedSequence { width: w, height: h, fps, frames: Vec::new() };
    }

    let sw = pres.slide_size.width.max(1.0);
    let sh = pres.slide_size.height.max(1.0);
    let scale = (f64::from(w) / sw).min(f64::from(h) / sh);
    let off_x = ((f64::from(w) - sw * scale) * 0.5).max(0.0);
    let off_y = ((f64::from(h) - sh * scale) * 0.5).max(0.0);
    let now = deckcraft_text::datetime::DateTime::now_local();
    let ropts = RenderOpts {
        scale,
        offset: (off_x, off_y),
        size: Some((w, h)),
        clear: Some(Rgba::BLACK),
        threads: 4,
        now,
        ..Default::default()
    };

    let hold_default = if opts.hold_secs.is_finite() { opts.hold_secs.clamp(0.05, 600.0) } else { 2.0 };
    let step_pause = if opts.step_pause_secs.is_finite() { opts.step_pause_secs.clamp(0.0, 30.0) } else { 0.35 };
    let aspect = sw / sh;

    let mut frames: Vec<SequenceFrame> = Vec::new();
    let mut prev_final: Option<(usize, Image)> = None;

    for (pos, &idx) in slides.iter().enumerate() {
        let Some(slide) = pres.slides.get(idx) else { continue };
        let tl = if opts.include_builds { slide_timeline(pres, idx) } else { Timeline::default() };
        let initial_img = render_slide_at_timeline(pres, idx, &tl, 0, 0.0, &ropts);

        // 1. Inter-slide transition (when configured on `slide` and either `pos > 0` or the first
        //    slide explicitly carries a non-none transition from black).
        if opts.include_transitions
            && let Some(tr) = slide.transition.as_ref()
            && !tr.kind.is_empty()
            && tr.kind != "none"
            && (pos > 0 || tr.duration_ms > 0)
        {
            let dur_secs = (f64::from(tr.duration_ms) / 1000.0).clamp(0.0, 30.0);
            let n_trans = if tr.kind == "cut" {
                0
            } else {
                ((dur_secs * f64::from(fps)).round() as u32).max(if dur_secs > 0.0 { 2 } else { 0 })
            };
            if n_trans > 0 {
                let prev_idx = prev_final.as_ref().map(|(pi, _)| *pi);
                if tr.kind == "morph" && prev_idx.is_some() {
                    for k in 0..n_trans {
                        let progress = (f64::from(k) + 0.5) / f64::from(n_trans);
                        let img = render_transition_between(pres, prev_idx, idx, progress, &ropts);
                        push_coalesced(&mut frames, img, 1, idx);
                    }
                } else {
                    let old_ref = prev_final.as_ref().map(|(_, img)| img);
                    let parts = uses_slide_parts(&tr.kind, &tr.option).then(|| SlideParts::render(pres, prev_idx, idx, &ropts));
                    for k in 0..n_trans {
                        let progress = (f64::from(k) + 0.5) / f64::from(n_trans);
                        let layers = transition_frame(&tr.kind, &tr.option, progress, aspect);
                        let img = composite_transition(old_ref, &initial_img, parts.as_ref(), &layers);
                        push_coalesced(&mut frames, img, 1, idx);
                    }
                }
            }
        }

        // 2. Intra-slide build steps (`Timeline`).
        if !tl.is_empty() {
            // Brief initial hold before the first manual click build if step 0 is `OnClick`.
            if !tl.step_is_auto(0) && step_pause > 0.0 {
                let pause_ticks = ((step_pause * f64::from(fps)).round() as u32).max(1);
                push_coalesced(&mut frames, initial_img.clone(), pause_ticks, idx);
            }
            for step in 0..tl.steps() {
                if step > 0 && !tl.step_is_auto(step) && step_pause > 0.0 {
                    let pause_ticks = ((step_pause * f64::from(fps)).round() as u32).max(1);
                    let settled = render_slide_at_timeline(pres, idx, &tl, step, 0.0, &ropts);
                    push_coalesced(&mut frames, settled, pause_ticks, idx);
                }
                let dur = tl.step_duration(step).clamp(0.0, 60.0);
                let step_ticks = ((dur * f64::from(fps)).ceil() as u32).max(1);
                for k in 1..=step_ticks {
                    let t = (f64::from(k) * dt).min(dur.max(dt));
                    let img = render_slide_at_timeline(pres, idx, &tl, step, t, &ropts);
                    push_coalesced(&mut frames, img, 1, idx);
                }
            }
        }

        // 3. Final settled slide state + slide hold duration.
        let final_img = if tl.is_empty() {
            initial_img
        } else {
            render_slide_at_timeline(pres, idx, &tl, tl.steps(), 0.0, &ropts)
        };
        let slide_hold = slide
            .transition
            .as_ref()
            .and_then(|tr| tr.advance_after_ms)
            .filter(|_| opts.use_slide_timings)
            .map(|ms| (f64::from(ms) / 1000.0).clamp(0.05, 600.0))
            .unwrap_or(hold_default);
        let hold_ticks = ((slide_hold * f64::from(fps)).round() as u32).max(1);
        push_coalesced(&mut frames, final_img.clone(), hold_ticks, idx);
        prev_final = Some((idx, final_img));
    }

    RenderedSequence { width: w, height: h, fps, frames }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;

    use deckcraft_model::anim::{AnimClass, AnimStart, Animation, Transition};
    use deckcraft_model::text::TextBody;
    use deckcraft_model::{Background, ColorRef, Fill, Geom, Presentation, Rgba, Shape, ShapeId, ShapeKind, Xfrm, defaults};

    use super::*;

    fn two_slide_deck(transition: Transition) -> Presentation {
        let mut pres = defaults::new_presentation(None);
        let layout = pres.slides[0].layout;
        let slide2 = defaults::new_slide(&mut pres, layout).expect("slide2");
        pres.slides.push(Arc::new(slide2));

        let s0 = Arc::make_mut(&mut pres.slides[0]);
        s0.background = Some(Background::Fill { fill: Fill::Solid { color: ColorRef::rgb(Rgba::rgb(220, 32, 32)) } });
        s0.shapes = vec![Shape {
            id: ShapeId(500),
            name: "!!HeroBox".into(),
            xfrm: Some(Xfrm::new(60.0, 120.0, 220.0, 160.0)),
            geom: Geom::preset("rect"),
            kind: ShapeKind::Shape,
            fill: Some(Fill::Solid { color: ColorRef::rgb(Rgba::rgb(255, 220, 40)) }),
            ..Default::default()
        }];

        let s1 = Arc::make_mut(&mut pres.slides[1]);
        s1.background = Some(Background::Fill { fill: Fill::Solid { color: ColorRef::rgb(Rgba::rgb(24, 96, 220)) } });
        s1.transition = Some(transition);
        s1.shapes = vec![Shape {
            id: ShapeId(501),
            name: "!!HeroBox".into(),
            xfrm: Some(Xfrm::new(620.0, 220.0, 260.0, 180.0)),
            geom: Geom::preset("roundRect"),
            kind: ShapeKind::Shape,
            fill: Some(Fill::Solid { color: ColorRef::rgb(Rgba::rgb(40, 230, 120)) }),
            ..Default::default()
        }];
        pres
    }

    #[test]
    fn transitions_produce_distinct_intermediate_frames_across_all_core_families() {
        let ropts = RenderOpts { scale: 0.2, size: Some((192, 108)), ..Default::default() };
        let cases = [
            ("fade", ""),
            ("push", "l"),
            ("push", "r"),
            ("push", "u"),
            ("push", "d"),
            ("wipe", "l"),
            ("wipe", "r"),
            ("wipe", "u"),
            ("wipe", "d"),
            ("split", "out"),
            ("uncover", "l"),
            ("cover", "r"),
            ("zoom", "in"),
            ("dissolve", ""),
            ("morph", ""),
        ];
        for (kind, option) in cases {
            let tr = Transition {
                kind: kind.into(),
                option: option.into(),
                duration_ms: 600,
                advance_on_click: true,
                advance_after_ms: None,
                ..Default::default()
            };
            let pres = two_slide_deck(tr);
            let f0 = render_transition_between(&pres, Some(0), 1, 0.0, &ropts);
            let fm = render_transition_between(&pres, Some(0), 1, 0.5, &ropts);
            let f1 = render_transition_between(&pres, Some(0), 1, 1.0, &ropts);
            assert_eq!((fm.width, fm.height), (192, 108));
            assert_ne!(fm.pixels, f0.pixels, "transition {kind}:{option} at t=0.5 matched t=0.0");
            assert_ne!(fm.pixels, f1.pixels, "transition {kind}:{option} at t=0.5 matched t=1.0");
        }
    }

    #[test]
    fn render_sequence_steps_builds_and_transitions_and_skips_hidden_slides() {
        let mut pres = two_slide_deck(Transition {
            kind: "push".into(),
            option: "l".into(),
            duration_ms: 500,
            advance_on_click: true,
            advance_after_ms: None,
            ..Default::default()
        });
        let layout = pres.slides[0].layout;
        let mut hidden = defaults::new_slide(&mut pres, layout).expect("hidden slide");
        hidden.hidden = true;
        hidden.background = Some(Background::Fill { fill: Fill::Solid { color: ColorRef::rgb(Rgba::rgb(10, 200, 10)) } });
        pres.slides.insert(1, Arc::new(hidden));

        // Add an intra-slide fade entrance animation on the final visible slide (index 2).
        let s2 = Arc::make_mut(&mut pres.slides[2]);
        s2.shapes.push(Shape {
            id: ShapeId(777),
            xfrm: Some(Xfrm::new(80.0, 80.0, 300.0, 120.0)),
            geom: Geom::preset("rect"),
            kind: ShapeKind::Shape,
            fill: Some(Fill::Solid { color: ColorRef::rgb(Rgba::WHITE) }),
            text: Some(TextBody::from_text("Animated Build")),
            ..Default::default()
        });
        s2.animations.push(Animation {
            shape: ShapeId(777),
            class: AnimClass::Entrance,
            effect: "fade".into(),
            option: String::new(),
            start: AnimStart::AfterPrevious,
            duration_ms: 400,
            delay_ms: 0,
            ..Default::default()
        });

        let seq = render_sequence(
            &pres,
            &SequenceOptions {
                width: 160,
                height: None,
                even_dimensions: true,
                fps: 10,
                hold_secs: 0.5,
                step_pause_secs: 0.1,
                include_hidden: false,
                ..Default::default()
            },
        );
        assert_eq!((seq.width, seq.height), (160, 90));
        // Hidden slide index 1 must never appear in emitted frames.
        assert!(seq.frames.iter().all(|f| f.slide != 1), "hidden slide should be skipped by default");
        // 1 hold frame on slide 0 + ~5 push transition frames + ~4 fade build frames + hold frame on slide 2.
        assert!(seq.frames.len() >= 8, "expected multi-frame transition+build sequence, got {}", seq.frames.len());
        assert!(seq.total_duration_ms() >= 1300);
    }
}
