use super::{
    Action, Document, Easing, FrameTex, HashSet, NodeId, NodeKind, OverlayAnchor, OverlayPosition, OverlayState, PaintKind, Presenter, Rect,
    ScrollAnim, Temp, Transition, Trigger, VideoPlaybackState, Waiting,
};

#[derive(Clone, Copy)]
struct ActionTiming {
    transition: Transition,
    duration: f64,
    easing: Easing,
    rect: Option<Rect>,
    now: f64,
    depth: usize,
}

impl Presenter {
    /// A mouse enter, leave, down or up on `node` (shown on `root`) met its interaction `k`: its
    /// actions run now, or after its delay while `root` is still what shows (again once only
    /// while it waits).
    pub(super) fn gesture(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64, root: NodeId) -> bool {
        let delay = self.doc.as_ref().and_then(|d| d.node(node)).and_then(|n| n.interactions.get(k)).map_or(0.0, |it| it.delay_secs());
        if delay <= 0.0 {
            return self.fire(node, k, rect, now);
        }
        if !self.waiting.iter().any(|w| (w.node, w.k) == (node, k)) {
            self.waiting.push(Waiting { node, k, rect, root, due: now + delay });
        }
        false
    }

    /// Run the delayed mouse triggers that are due (those whose screen or overlay went away are
    /// dropped). True when one changed what shows.
    pub(super) fn fire_waiting(&mut self, root: NodeId, now: f64) -> bool {
        let (due, later): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting).into_iter().partition(|w| w.due <= now);
        self.waiting = later;
        self.waiting.retain(|w| w.root == root);
        let mut did = false;
        for w in due.into_iter().filter(|w| w.root == root) {
            // An earlier one may have left the screen: the rest are dropped with it. The gesture
            // is long over: what these open is not the viewer's doing (#1096).
            did = did || self.fire_from(w.node, w.k, w.rect, now, false);
        }
        did
    }

    /// Run the actions of interaction `k` on `node`. `rect` is the layer's screen rect.
    pub(crate) fn fire(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64) -> bool {
        self.fire_from(node, k, rect, now, true)
    }

    /// [`fire`](Self::fire); `live`: the viewer's input fired it just now (not a delay it waited
    /// out). The links it opens are the viewer's own only when live from a click, press, drag or
    /// key: a delay, the pointer passing over a layer and a timed mouse trigger fire without the
    /// viewer asking for anything ([`Presenter::urls`], #1096).
    fn fire_from(&mut self, node: NodeId, k: usize, rect: Option<Rect>, now: f64, live: bool) -> bool {
        let Some(it) = self.doc.as_ref().and_then(|d| d.node(node)).and_then(|n| n.interactions.get(k)).cloned() else { return false };
        let dur = it.easing.seconds(it.duration_ms);
        let temp = match it.trigger {
            Trigger::OnHover => Some(false),
            Trigger::OnPress => Some(true),
            _ => None,
        };
        let snap = if temp.is_some() && self.temp.is_none() { self.snapshot() } else { None };
        let mut did = false;
        let first_link = self.urls.len();
        for a in &it.actions {
            did |= self.perform(a, node, it.transition, dur, it.easing, rect, now);
        }
        let asked = live
            && matches!(
                it.trigger,
                Trigger::OnClick | Trigger::OnPress | Trigger::OnDrag | Trigger::MouseDown | Trigger::MouseUp | Trigger::KeyPress { .. }
            );
        for link in self.urls.iter_mut().skip(first_link) {
            link.1 = asked;
        }
        if did && let (Some(press), Some(snap), Some(rect)) = (temp, snap, rect) {
            self.temp = Some(Temp { rect, press, snap, transition: it.transition, dur, easing: it.easing });
        }
        did
    }

    /// Run the widget clicks the player took: each calls its plugin back on the player's copy (the
    /// prototype's own state of the widget), as on the canvas but without touching the file.
    /// Returns what went wrong, for a toast.
    pub fn click_widgets(&mut self, s: &figcraft_engine::Session) -> Option<String> {
        let mut error = None;
        for (id, target) in std::mem::take(&mut self.widget_clicks) {
            let Some(doc) = self.design.clone() else { break };
            match figcraft_engine::widgets::click_on(s, doc, id, &target) {
                Ok((d, _)) => {
                    self.design = Some(d);
                    self.relayout();
                }
                Err(e) => error = Some(e.to_string()),
            }
        }
        error
    }

    pub(crate) fn perform(&mut self, a: &Action, node: NodeId, tr: Transition, dur: f64, easing: Easing, rect: Option<Rect>, now: f64) -> bool {
        self.perform_at(a, node, ActionTiming { transition: tr, duration: dur, easing, rect, now, depth: 0 })
    }

    /// One action; `depth` counts the conditional actions it is inside.
    fn perform_at(&mut self, a: &Action, node: NodeId, timing: ActionTiming) -> bool {
        let ActionTiming { transition: tr, duration: dur, easing, rect, now, depth } = timing;
        let Some(doc) = self.doc.clone() else { return false };
        match a {
            Action::Navigate { dest, preserve_scroll } => {
                let Some(top) = figcraft_engine::proto_run::navigate_target(&doc, *dest, &self.sections) else { return false };
                if !doc.contains(top) || (top == self.current && self.overlays.is_empty()) {
                    return false;
                }
                self.go(top, tr, dur, easing, now, true, *preserve_scroll);
                true
            }
            Action::Back => {
                if self.close_overlay(now) {
                    return true;
                }
                match self.history.pop() {
                    Some(v) => {
                        self.go(v.frame, v.transition.reversed(), v.dur, v.easing, now, false, false);
                        true
                    }
                    None => false,
                }
            }
            Action::ScrollTo { dest, offset } => {
                let frames = self.scroll_to_layer(&doc, *dest, *offset);
                if frames.is_empty() {
                    return false;
                }
                if matches!(tr, Transition::Instant) || dur <= 0.0 {
                    self.scroll.extend(frames.iter().map(|(f, _, to)| (*f, *to)));
                    self.rev += 1;
                } else {
                    self.scroll_anim = Some(ScrollAnim { frames, start: now, dur, easing });
                }
                true
            }
            Action::OpenOverlay { dest, position, close_on_outside, background, offset, anchor } => {
                let at = self.page_rect(rect);
                if !doc.contains(*dest) {
                    return false;
                }
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                self.overlays.push(OverlayState {
                    id: *dest,
                    position: *position,
                    close_on_outside: *close_on_outside,
                    background: *background,
                    anchor: at,
                    offset: *offset,
                    anchor_to: *anchor,
                    opened: now,
                    transition: tr,
                    dur,
                    easing,
                });
                self.hovered.clear();
                true
            }
            Action::SwapOverlay { dest } => {
                let at = self.page_rect(rect);
                if !doc.contains(*dest) {
                    return false;
                }
                self.delays_fired.retain(|(id, _)| !doc.is_ancestor(*dest, *id));
                match self.overlays.last_mut() {
                    Some(o) => {
                        o.id = *dest;
                        o.opened = now;
                        o.transition = tr;
                        o.dur = dur;
                        o.easing = easing;
                    }
                    None => self.overlays.push(OverlayState {
                        id: *dest,
                        position: OverlayPosition::Center,
                        close_on_outside: true,
                        background: None,
                        anchor: at,
                        offset: None,
                        anchor_to: OverlayAnchor::Screen,
                        opened: now,
                        transition: tr,
                        dur,
                        easing,
                    }),
                }
                self.hovered.clear();
                true
            }
            Action::CloseOverlay => self.close_overlay(now),
            Action::OpenUrl { url, new_tab } => {
                // The URL comes from the document and this action fires from whatever trigger the
                // prototype sets, including a delay, so no click is implied: only a web or mail
                // link is kept, as not the viewer's doing until `fire_from` says otherwise.
                if figcraft_engine::doc_urls::openable(url) {
                    self.urls.push((url.clone(), false, *new_tab));
                }
                true
            }
            Action::ChangeTo { dest } => {
                let Some(inst) = figcraft_engine::proto_run::change_to_target(&doc, node, *dest) else { return false };
                if !doc.contains(*dest) {
                    return false;
                }
                self.begin(tr, dur, easing, now);
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                if let Some(n) = d.node_mut(inst)
                    && let NodeKind::Instance { component, .. } = &mut n.kind
                {
                    *component = *dest;
                }
                figcraft_engine::components::sync_instance(d, inst);
                self.settle(&before);
                true
            }
            Action::SetVariable { variable, value, expr } => {
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                let set = figcraft_engine::proto_run::set_variable(d, node, variable, value, expr.as_ref());
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::SetVariableMode { collection, mode } => {
                let Some(d) = self.design.as_mut() else { return false };
                let before = d.clone();
                let set = figcraft_engine::proto_run::set_mode_by_id(d, self.current, collection, mode);
                if set {
                    self.settle(&before);
                }
                set
            }
            Action::Conditional { blocks } => {
                if depth >= figcraft_doc::proto::MAX_ACTION_DEPTH {
                    return false;
                }
                // Evaluated now: earlier actions of the interaction may have changed variables.
                let mut did = false;
                for inner in figcraft_engine::proto_run::branch(&doc, node, blocks) {
                    did |= self.perform_at(&inner, node, ActionTiming { depth: depth + 1, ..timing });
                }
                did
            }
        }
    }

    pub(super) fn texture(&mut self, ctx: &egui::Context, doc: &Document, id: NodeId, px_scale: f64, now: f64) -> Option<FrameTex> {
        // Animated GIFs play from when the frame arrived: a new texture when one of them moves on,
        // and a wake-up for that moment (nothing else draws frames while it shows). egui wakes a
        // display frame before the time asked for, a little earlier still at times: ask that much
        // later, and count a frame due within a few milliseconds as due.
        let t = (now - self.arrived).max(0.0) + 0.005;
        let clock = figcraft_render::images::animation_clock(doc, id, t);
        if let Some((_, next)) = clock {
            crate::repaint_in(ctx, next + f64::from(ctx.input(|i| i.predicted_dt)));
        }
        let key = (self.rev, px_scale.to_bits(), clock.map_or(0, |c| c.0));
        if let Some(t) = self.textures.get(&id)
            && t.key == key
        {
            return Some(t.clone());
        }
        let bounds = figcraft_render::export_bounds(doc, id)?;
        let pix = figcraft_render::render_node_at(doc, id, texture_scale(ctx, bounds, px_scale), t)?;
        let img = crate::render_worker::screen_image(&pix, doc.color_profile);
        let tex = ctx.load_texture(format!("present-{}", id.0), img, egui::TextureOptions::LINEAR);
        let ft = FrameTex { key, tex, bounds };
        self.textures.insert(id, ft.clone());
        Some(ft)
    }

    /// Visible `(node_id, fill_index, video_paint)` layers in the presented screen and open overlays.
    pub fn active_video_layers(&self) -> Vec<(NodeId, usize, figcraft_color::VideoPaint)> {
        let Some(doc) = self.doc.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        let roots = std::iter::once(self.current).chain(self.overlays.iter().map(|o| o.id));
        for root in roots {
            let mut hidden: HashSet<NodeId> = HashSet::new();
            for id in doc.subtree(root) {
                let Some(n) = doc.node(id) else { continue };
                if !n.visible || (id != root && n.parent.is_some_and(|p| hidden.contains(&p))) {
                    hidden.insert(id);
                    continue;
                }
                for (fi, fill) in n.fills.iter().enumerate() {
                    if fill.visible
                        && let PaintKind::Video(vp) = &fill.kind
                    {
                        out.push((id, fi, vp.clone()));
                    }
                }
            }
        }
        out
    }

    /// Synchronise `self.video_playback` with the visible video fills on the active screen/overlays.
    pub fn sync_video_playback(&mut self) {
        let active = self.active_video_layers();
        let Some(doc) = self.doc.as_ref() else {
            self.video_playback.clear();
            return;
        };
        let mut keep: HashSet<(NodeId, usize)> = HashSet::with_capacity(active.len());
        for (id, fi, vp) in active {
            keep.insert((id, fi));
            let duration = doc.videos.get(&vp.video).map(|v| v.duration_secs).filter(|d| d.is_finite() && *d > 0.0).unwrap_or(5.0).max(0.1);
            self.video_playback
                .entry((id, fi))
                .and_modify(|st| {
                    st.duration = duration;
                    st.loop_playback = vp.loop_playback;
                })
                .or_insert(VideoPlaybackState {
                    playing: vp.autoplay,
                    muted: vp.muted,
                    position: 0.0,
                    duration,
                    loop_playback: vp.loop_playback,
                    last_tick: None,
                });
        }
        self.video_playback.retain(|k, _| keep.contains(k));
    }

    /// Advance playing video fills on the UI clock `now`. Returns true while at least one plays.
    pub fn step_videos(&mut self, now: f64) -> bool {
        let mut any_playing = false;
        for st in self.video_playback.values_mut() {
            if !st.playing {
                st.last_tick = None;
                continue;
            }
            let dt = st.last_tick.map_or(0.0, |last| (now - last).clamp(0.0, 1.0));
            st.position += dt;
            let dur = st.duration.max(0.1);
            if st.position >= dur {
                if st.loop_playback {
                    st.position %= dur;
                } else {
                    st.position = dur;
                    st.playing = false;
                }
            }
            st.last_tick = st.playing.then_some(now);
            any_playing |= st.playing;
        }
        any_playing
    }

    /// Play or pause a video fill; restarting from the end rewinds to 0.0.
    pub fn toggle_video_playback(&mut self, node: NodeId, fill_index: usize, now: f64) -> Option<bool> {
        let st = self.video_playback.get_mut(&(node, fill_index))?;
        if !st.playing && st.position >= st.duration {
            st.position = 0.0;
        }
        st.playing = !st.playing;
        st.last_tick = st.playing.then_some(now);
        Some(st.playing)
    }

    /// Mute or unmute a video fill in the prototype player.
    pub fn toggle_video_mute(&mut self, node: NodeId, fill_index: usize) -> Option<bool> {
        let st = self.video_playback.get_mut(&(node, fill_index))?;
        st.muted = !st.muted;
        Some(st.muted)
    }

    /// Seek a video fill to `ratio` in `[0.0, 1.0]`, returning the new position in seconds.
    pub fn seek_video(&mut self, node: NodeId, fill_index: usize, ratio: f64) -> Option<f64> {
        let st = self.video_playback.get_mut(&(node, fill_index))?;
        let dur = st.duration.max(0.1);
        st.position = (ratio.clamp(0.0, 1.0) * dur).clamp(0.0, dur);
        Some(st.position)
    }

    /// Multi-mode variable collections available for live switching in the prototype player:
    /// `(collection_id, collection_name, active_mode_id, [(mode_id, mode_name)])`.
    pub fn variable_collections(&self) -> Vec<(String, String, String, Vec<(String, String)>)> {
        let Some(doc) = self.doc.as_ref() else { return Vec::new() };
        let active_modes = doc.modes_for(self.current);
        doc.variables
            .collections
            .iter()
            .filter(|c| c.modes.len() > 1)
            .map(|c| {
                let active = active_modes.get(&c.id).cloned().or_else(|| c.default_mode().cloned()).unwrap_or_default();
                let modes = c.modes.iter().map(|m| (m.id.clone(), m.name.clone())).collect();
                (c.id.clone(), c.name.clone(), active, modes)
            })
            .collect()
    }

    /// Switch `collection_id` to `mode_id` live on the player's document copy, re-evaluating all
    /// variable-bound colours, numbers, strings and booleans immediately.
    pub fn switch_variable_mode(&mut self, collection_id: &str, mode_id: &str) -> bool {
        let Some(d) = self.design.as_mut() else { return false };
        let before = d.clone();
        let set = figcraft_engine::proto_run::switch_screen_mode(d, self.current, collection_id, mode_id);
        if set {
            self.settle(&before);
            self.sync_video_playback();
        }
        set
    }

    /// True when any variable collection's mode on the player's copy differs from the editor's design.
    pub fn has_variable_mode_overrides(&self) -> bool {
        let (Some(cur), Some(orig)) = (self.design.as_ref(), self.original.as_ref()) else { return false };
        orig.nodes().any(|on| cur.node(on.id).is_some_and(|cn| cn.var_modes != on.var_modes))
    }

    /// Reset all prototype variable mode switches back to the document's authored default modes.
    pub fn reset_variable_modes(&mut self) -> bool {
        let (Some(d), Some(orig)) = (self.design.as_mut(), self.original.as_ref()) else { return false };
        let before = d.clone();
        let changed = figcraft_engine::proto_run::reset_modes(d, orig);
        if changed {
            self.settle(&before);
            self.sync_video_playback();
        }
        changed
    }
}

/// The most pixels a presented frame's texture holds (bigger prototypes, at 100% or filling the
/// window, are drawn from a smaller texture scaled up).
const MAX_TEXTURE_PIXELS: f64 = 16_777_216.0;

/// The pixel scale, at most `px`, at which `bounds` makes a texture the GPU takes (no side past
/// its limit) of at most [`MAX_TEXTURE_PIXELS`].
pub(super) fn texture_scale(ctx: &egui::Context, bounds: figcraft_geom::Rect, px: f64) -> f64 {
    let side = ctx.input(|i| i.max_texture_side) as f64 - 2.0;
    let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
    let s = px.min(side / w.max(h)).min((MAX_TEXTURE_PIXELS / (w * h)).sqrt());
    if s.is_finite() && s > 0.0 { s } else { 1e-3 }
}

/// The first interaction index on `id` whose trigger satisfies `want`.
pub(super) fn find_trigger(doc: &Document, id: NodeId, want: impl Fn(&Trigger) -> bool) -> Option<usize> {
    doc.node(id)?.interactions.iter().position(|it| want(&it.trigger))
}
