use super::chrome::{BarAction, layer_at, paint_menu_chevron};
use super::device::{device_body, device_glass};
use super::draw::draw;
use super::input::{InputMode, input};
use super::layout::{Chrome, Layout, layout};
use super::navigate::next_delay;
use super::scroll::page_settings;
use super::transition::lerp;
use super::{
    ACCENT, Align2, Arc, Color32, CornerRadius, Device, Icon, NodeKind, PresentScale, Presenter, Rect, Sense, Stroke, StrokeKind, Tip, Tokens, Ui,
    icons, pos2, regular, semibold, tr, tr_noop, vec2,
};

/// One frame of the player in `rect`: the top bar, then the prototype (on its device) below.
/// Returns what the bar asked for (restarting is done here).
pub(super) fn player(p: &mut Presenter, ui: &mut Ui, rect: Rect, now: f64, chrome: Chrome) -> Option<BarAction> {
    let ctx = ui.ctx().clone();
    p.clock = now;
    // Hide UI (the full-window presenter only): no bar, the prototype takes the whole window.
    let hidden = p.view.hide_ui && !chrome.pane;
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), if hidden { 0.0 } else { chrome.bar_h }));
    let area = Rect::from_min_max(pos2(rect.min.x, bar.max.y), rect.max);
    let bg = crate::theme::doc_c32(ui.ctx(), p.doc.as_ref().map(|d| page_settings(d, p.current)).unwrap_or_default().background);
    let r = chrome.round;
    ui.painter().rect_filled(area, CornerRadius { nw: 0, ne: 0, sw: r, se: r }, bg);
    let action = if hidden { None } else { top_bar(p, ui, bar, now, chrome) };
    if action == Some(BarAction::Restart) {
        p.restart(p.start, now);
    }
    if p.doc.as_ref().is_none_or(|d| !d.contains(p.current)) {
        // Controller buttons pressed with no frame to show are dropped, as they are while a link
        // notice shows.
        p.pads.clear();
        if !chrome.pane {
            return Some(BarAction::Close);
        }
        ui.painter().text(area.center(), Align2::CENTER_CENTER, tr("Select a frame to preview it"), regular(12.0), Color32::from_gray(170));
        return action;
    }
    // Scroll animation (Scroll to).
    if let Some(sa) = &p.scroll_anim {
        let raw = ((now - sa.start) / sa.dur.max(1e-3)).clamp(0.0, 1.0);
        let e = sa.easing.eval(raw);
        for (f, from, to) in &sa.frames {
            p.scroll.insert(*f, (lerp(from.0, to.0, e), lerp(from.1, to.1, e)));
        }
        p.rev += 1;
        if raw >= 1.0 {
            p.scroll_anim = None;
        }
        ctx.request_repaint();
    }
    // Scrolling that carries on after the input let go (repaints only while it moves).
    if p.step_fling(now) {
        ctx.request_repaint();
    }
    p.sync_video_playback();
    if p.step_videos(now) {
        ctx.request_repaint();
    }
    let resp = ui.interact(area, ui.scope_id().with("present_area"), Sense::click_and_drag());
    let over = resp.contains_pointer();
    if let Some(l) = layout(p, area, chrome) {
        // The pane only takes the keyboard while the pointer is over it. While a link notice shows
        // the prototype takes neither keys nor the pointer (Escape there cancels the notice).
        let keys = chrome.keys && p.link_notice.is_none() && (!chrome.pane || over);
        input(p, &ctx, &l, InputMode { over_area: over && p.link_notice.is_none(), keys_on: keys, pane: chrome.pane, now });
    } else {
        p.pads.clear();
    }
    let Some(l) = layout(p, area, chrome) else { return action };
    p.shown = Some((p.current, l.place, l.clip.intersect(area)));
    let painter = ui.painter_at(area);
    let body = l.device.zip(l.color);
    if l.device.is_some() {
        if let Some((d, c)) = body {
            device_body(&painter, d.name, l.screen, l.landscape, c);
        }
        painter.rect_filled(l.screen, CornerRadius::ZERO, bg);
    }
    draw(p, &painter, &ctx, &l, now);
    video_controls(p, ui, &painter, &l, now);
    if p.view.inspect && !chrome.pane {
        inspect_hover(p, &painter, &ctx, &l, area);
    }
    if let Some((d, _)) = body {
        device_glass(&painter, d.name, l.screen, l.landscape);
    }
    // Timers: wake up for the soonest pending "after delay" of the layer that can fire it (they
    // wait while a transition plays, which draws every frame anyway).
    if p.anim.is_none()
        && let Some((_, due)) = next_delay(p, &l.disp, l.active(p.current).0, now)
    {
        crate::repaint_in(&ctx, due.min(60.0));
    }
    // And for the soonest mouse trigger waiting out its delay.
    if let Some(due) = p.waiting.iter().map(|w| w.due - now).reduce(f64::min) {
        crate::repaint_in(&ctx, due.clamp(0.0, 60.0));
    }
    action
}

/// Inspecting: the layer under the pointer becomes the inspected one; it is outlined.
fn inspect_hover(p: &mut Presenter, painter: &egui::Painter, ctx: &egui::Context, l: &Layout, area: Rect) {
    let (root, place, clip) = l.active(p.current);
    if let Some(q) = ctx.pointer_hover_pos().filter(|q| area.contains(*q) && clip.contains(*q))
        && let Some(id) = layer_at(&l.disp, root, &place, q)
    {
        p.inspected = Some(id);
    }
    if let Some(id) = p.inspected.filter(|id| *id == root || l.disp.is_ancestor(root, *id))
        && let Some(b) = l.disp.world_bounds(id)
    {
        painter.with_clip_rect(clip).rect_stroke(place.rect(b), CornerRadius::ZERO, Stroke::new(1.5, ACCENT), StrokeKind::Outside);
    }
}

/// The bar above the prototype: the flow picker and the frame's name on the left, the buttons on
/// the right (text on the dark full-window bar, icons in the pane, which follows the theme).
fn top_bar(p: &mut Presenter, ui: &mut Ui, bar: Rect, now: f64, chrome: Chrome) -> Option<BarAction> {
    let t = Tokens::get(ui.ctx());
    let (fill, text, dim, hover) = (t.panel, t.text, t.text_secondary, t.hover);
    let r = chrome.round;
    ui.painter().rect_filled(bar, CornerRadius { nw: r, ne: r, sw: 0, se: 0 }, fill);
    if chrome.pane {
        ui.painter().line_segment([bar.left_bottom(), bar.right_bottom()], Stroke::new(1.0, t.border));
    }
    let source_key = p.source.as_ref().map_or(0, |s| Arc::as_ptr(s) as usize);
    if !chrome.pane && p.bar_thumb.as_ref().is_none_or(|(id, source, _)| *id != p.current || *source != source_key) {
        p.bar_thumb =
            p.source.as_ref().and_then(|d| crate::assets_panel::thumbnail(ui.ctx(), d, p.current, 38.0)).map(|tex| (p.current, source_key, tex));
    }
    let doc = p.doc.as_ref()?;
    let name = doc.node(p.current).map(|n| n.name.clone()).unwrap_or_default();
    let flows = match doc.page_of(p.start).and_then(|pg| doc.node(pg)).map(|n| &n.kind) {
        Some(NodeKind::Page { flows, .. }) => flows.clone(),
        _ => vec![],
    };
    let settings = page_settings(doc, p.current);
    let device = settings.device();
    let mut action = None;
    // Right: the buttons, right to left (English labels key their ids; shown translated).
    let buttons: &[(BarAction, &str, Icon)] = if chrome.pane {
        &[
            (BarAction::Close, tr_noop("Close preview"), Icon::Close),
            (BarAction::Present, tr_noop("Present"), Icon::Play),
            (BarAction::Restart, tr_noop("Restart"), Icon::Reset),
        ]
    } else {
        &[
            (BarAction::Close, tr_noop("Close"), Icon::Close),
            (BarAction::Restart, tr_noop("Restart"), Icon::Reset),
            (BarAction::Options, tr_noop("Options"), Icon::Dots),
            (BarAction::Comments, tr_noop("Comments"), Icon::Comment),
            (BarAction::Inspect, tr_noop("Inspect"), Icon::Code),
        ]
    };
    let mut x = bar.max.x - if chrome.pane { 6.0 } else { 8.0 };
    for (act, label, icon) in buttons {
        let shown = tr(label);
        let w = if chrome.pane { 24.0 } else { ui.painter().layout_no_wrap(shown.to_string(), regular(11.5), text).size().x + 16.0 };
        let br = Rect::from_min_size(pos2(x - w, bar.center().y - 12.0), vec2(w, 24.0));
        x = br.min.x - if chrome.pane { 2.0 } else { 4.0 };
        let mut resp = ui.interact(br, ui.scope_id().with(("pbar", *label)), Sense::click());
        if chrome.pane {
            resp = resp.tip(shown);
        }
        let on = (*act == BarAction::Comments && p.view.comments) || (*act == BarAction::Inspect && p.view.inspect);
        if on {
            ui.painter().rect_filled(br, CornerRadius::same(5), t.accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(br, CornerRadius::same(5), hover);
        }
        if *act == BarAction::Comments {
            resp = resp.tip(tr("Show comments (Shift+C)"));
        }
        if *act == BarAction::Inspect {
            resp = resp.tip(tr("Inspect the layer under the pointer: size, colours, typography"));
        }
        if chrome.pane {
            icons::paint(ui.painter(), crate::widgets::icon_glyph_rect(br), *icon, if on { t.accent_on } else { t.icon });
        } else {
            ui.painter().text(br.center(), Align2::CENTER_CENTER, shown, regular(11.5), if on { t.accent_on } else { text });
        }
        if *act == BarAction::Options {
            let menu = crate::right::menu_popup(&resp).width(220.0).show(|ui| options_menu(p, ui, device, settings.responsive, now));
            action = action.or(menu.and_then(|m| m.inner));
        } else if resp.clicked() {
            action = Some(*act);
        }
    }
    // Live variable mode switcher right in the prototype player bar (both pane and full-window).
    if !p.variable_collections().is_empty() {
        let shown = tr("Switch variable mode");
        let w = if chrome.pane { 24.0 } else { ui.painter().layout_no_wrap(shown.to_string(), regular(11.5), text).size().x + 16.0 };
        let br = Rect::from_min_size(pos2(x - w, bar.center().y - 12.0), vec2(w, 24.0));
        x = br.min.x - if chrome.pane { 2.0 } else { 4.0 };
        let mut resp = ui.interact(br, ui.scope_id().with("pbar_var_modes"), Sense::click());
        if chrome.pane {
            resp = resp.tip(shown);
        }
        let on = p.has_variable_mode_overrides();
        if on {
            ui.painter().rect_filled(br, CornerRadius::same(5), t.accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(br, CornerRadius::same(5), hover);
        }
        if chrome.pane {
            icons::paint(ui.painter(), crate::widgets::icon_glyph_rect(br), Icon::Variable, if on { t.accent_on } else { t.icon });
        } else {
            ui.painter().text(br.center(), Align2::CENTER_CENTER, shown, regular(11.5), if on { t.accent_on } else { text });
        }
        crate::right::menu_popup(&resp).width(220.0).show(|ui| variable_modes_menu(p, ui));
    }
    // Left: the flow picker and the frame's name, clipped before the buttons.
    let left = ui.painter().with_clip_rect(Rect::from_min_max(bar.min, pos2(x, bar.max.y)));
    let mut lx = bar.min.x + if chrome.pane { 8.0 } else { 12.0 };
    if !chrome.pane {
        let thumb = Rect::from_center_size(pos2(lx + 15.0, bar.center().y), vec2(28.0, 28.0));
        left.rect_filled(thumb, CornerRadius::same(4), t.field);
        if let Some((_, _, tex)) = &p.bar_thumb {
            let size = tex.size_vec2();
            if size.x > 0.0 && size.y > 0.0 {
                let scaled = size * (26.0 / size.x.max(size.y));
                let dst = Rect::from_center_size(thumb.center(), scaled);
                left.image(tex.id(), dst, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
        lx += 38.0;
    }
    // The flow playing: its name on the picker, its description on hover; the menu lists each
    // flow with its description.
    let playing = flows.iter().find(|f| f.start == p.start);
    if !flows.is_empty() {
        let label = playing.map_or(tr("Flows"), |f| f.name.as_str()).to_owned();
        let tip = playing.map(|f| f.description.trim()).filter(|d| !d.is_empty()).unwrap_or(tr("Flows")).to_string();
        let galley = left.layout_no_wrap(label, semibold(11.5), text);
        let fr = Rect::from_min_size(pos2(lx, bar.center().y - 12.0), vec2(galley.size().x + 30.0, 24.0));
        let resp = ui.interact(fr.intersect(left.clip_rect()), ui.scope_id().with("pflows"), Sense::click()).tip(tip);
        if resp.hovered() {
            left.rect_filled(fr, CornerRadius::same(5), hover);
        }
        left.galley(pos2(fr.min.x + 8.0, fr.center().y - galley.size().y / 2.0), galley, text);
        paint_menu_chevron(&left, fr, t.icon_secondary);
        crate::right::menu_popup(&resp).width(240.0).show(|ui| {
            let t = Tokens::get(ui.ctx());
            for f in &flows {
                if crate::right::menu_check(ui, f.start == p.start, &f.name, "") {
                    p.restart(f.start, now);
                    ui.close();
                }
                let d = f.description.trim();
                if !d.is_empty() {
                    egui::Frame::NONE.inner_margin(egui::Margin { left: 26, right: 8, top: 0, bottom: 4 }).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(d).font(regular(10.5)).color(t.text_secondary)).wrap());
                    });
                }
            }
        });
        lx = fr.max.x + 8.0;
        left.text(pos2(lx, bar.center().y), Align2::LEFT_CENTER, "/", regular(11.5), dim);
        lx += 14.0;
    }
    left.text(pos2(lx, bar.center().y), Align2::LEFT_CENTER, name, regular(11.5), text);
    action
}

/// The full-window presenter's options menu: how the prototype is sized (Z cycles them), whether
/// the frame is responsive (a file setting: the frame takes the window's size, or on a device the
/// screen's, instead of being scaled or shown at a fixed size), the device's body, hotspot hints,
/// restart (R).
fn options_menu(p: &mut Presenter, ui: &mut Ui, device: Option<Device>, responsive: bool, now: f64) -> Option<BarAction> {
    use crate::right::menu_check;
    let mut action = None;
    let on_device = device.is_some();
    // Without a device, responsive is one of the sizes: the frame fills the window at 100%.
    if !on_device && menu_check(ui, responsive, tr("Responsive"), "") {
        action = Some(BarAction::Responsive(true));
        ui.close();
    }
    for s in PresentScale::options(on_device) {
        if menu_check(ui, p.view.scale == *s && (on_device || !responsive), s.label(on_device), "") {
            p.set_scale(*s);
            if !on_device && responsive {
                action = Some(BarAction::Responsive(false));
            }
            ui.close();
        }
    }
    if (on_device || !responsive) && menu_check(ui, false, tr("Next size option"), "Z") {
        p.set_scale(p.view.scale.next(on_device));
        ui.close();
    }
    ui.separator();
    // On a device: the frame takes the screen's size, or keeps its own (clipped or scrolling).
    if on_device {
        for (on, label) in [(true, tr("Responsive")), (false, tr("Fixed size"))] {
            if menu_check(ui, responsive == on, label, "") {
                action = (responsive != on).then_some(BarAction::Responsive(on));
                ui.close();
            }
        }
        ui.separator();
    }
    if device.is_some_and(|d| !d.colors.is_empty()) && menu_check(ui, p.view.device_frame, tr("Show device frame"), "") {
        p.view.device_frame = !p.view.device_frame;
        ui.close();
    }
    if menu_check(ui, p.view.hotspot_hints, tr("Show hotspot hints"), "") {
        p.view.hotspot_hints = !p.view.hotspot_hints;
        ui.close();
    }
    // Hide UI, as the reference tool's Options menu has it (#1820): the bars go away, the key
    // (the editor's Show/Hide UI key) brings them back.
    if menu_check(ui, p.view.hide_ui, tr("Hide UI"), &crate::toolbar::pretty_shortcut("Cmd+\\")) {
        action = Some(BarAction::HideUi);
        ui.close();
    }
    if menu_check(ui, false, tr("Restart"), "R") {
        p.restart(p.start, now);
        ui.close();
    }
    action
}

/// Menu listing every multi-mode variable collection and its modes for live switching inside the
/// prototype player, plus a Reset action when any mode override is active.
pub(super) fn variable_modes_menu(p: &mut Presenter, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let collections = p.variable_collections();
    for (idx, (cid, cname, active, modes)) in collections.iter().enumerate() {
        if idx > 0 {
            ui.separator();
        }
        ui.label(egui::RichText::new(cname).font(semibold(10.5)).color(t.menu_dim));
        for (mid, mname) in modes {
            if crate::right::menu_check(ui, active == mid, mname, "") {
                p.switch_variable_mode(cid, mid);
                ui.close();
            }
        }
    }
    if p.has_variable_mode_overrides() {
        ui.separator();
        if crate::right::menu_item(ui, tr("Reset variable modes"), "") {
            p.reset_variable_modes();
            ui.close();
        }
    }
}

/// Interactive playback HUD drawn over visible video fills on the active presented screen/overlay:
/// Play/Pause toggle, clickable scrub bar with `mm:ss` timestamp, and Mute toggle.
fn video_controls(p: &mut Presenter, ui: &mut Ui, painter: &egui::Painter, l: &Layout, now: f64) {
    let (root, place, clip) = l.active(p.current);
    let keys: Vec<(super::NodeId, usize)> = p
        .video_playback
        .keys()
        .copied()
        .filter(|(id, _)| *id == root || l.disp.is_ancestor(root, *id))
        .collect();
    if keys.is_empty() {
        return;
    }
    let hover_pos = ui.ctx().pointer_hover_pos();
    for (id, fi) in keys {
        let Some(bounds) = l.disp.world_bounds(id) else { continue };
        let vrect = place.rect(bounds).intersect(clip);
        if vrect.width() < 44.0 || vrect.height() < 32.0 {
            continue;
        }
        let Some(st) = p.video_playback.get(&(id, fi)).cloned() else { continue };
        let ratio = (st.position / st.duration.max(0.1)).clamp(0.0, 1.0) as f32;
        let hovered = hover_pos.is_some_and(|q| vrect.contains(q));
        let cp = painter.with_clip_rect(vrect);
        // Subtle bottom progress line while playing unhovered; full HUD when paused or hovered.
        if !hovered && st.playing {
            let line = Rect::from_min_max(pos2(vrect.min.x, vrect.max.y - 3.0), vrect.max);
            cp.rect_filled(line, CornerRadius::ZERO, Color32::from_black_alpha(90));
            let fill = Rect::from_min_max(line.min, pos2(line.min.x + line.width() * ratio, line.max.y));
            cp.rect_filled(fill, CornerRadius::ZERO, ACCENT);
            continue;
        }
        let bar_h = 26.0_f32.min(vrect.height() - 6.0);
        let hud = Rect::from_min_max(
            pos2(vrect.min.x + 6.0, vrect.max.y - bar_h - 6.0),
            pos2(vrect.max.x - 6.0, vrect.max.y - 6.0),
        );
        cp.rect_filled(hud, CornerRadius::same(6), Color32::from_black_alpha(175));
        let play_r = Rect::from_center_size(pos2(hud.min.x + 14.0, hud.center().y), vec2(20.0, 20.0));
        let play_tip = if st.playing { tr("Pause") } else { tr("Play video") };
        let play_resp = ui.interact(play_r, ui.scope_id().with(("vid_play", id, fi)), Sense::click()).tip(play_tip);
        if play_resp.hovered() {
            cp.rect_filled(play_r, CornerRadius::same(4), Color32::from_white_alpha(36));
        }
        icons::paint(
            &cp,
            Rect::from_center_size(play_r.center(), vec2(14.0, 14.0)),
            if st.playing { Icon::Pause } else { Icon::Play },
            Color32::WHITE,
        );
        if play_resp.clicked() {
            p.toggle_video_playback(id, fi, now);
            ui.ctx().request_repaint();
        }
        let mute_r = Rect::from_center_size(pos2(hud.max.x - 16.0, hud.center().y), vec2(22.0, 20.0));
        let mute_resp = ui.interact(mute_r, ui.scope_id().with(("vid_mute", id, fi)), Sense::click()).tip(tr("Muted|video"));
        if mute_resp.hovered() {
            cp.rect_filled(mute_r, CornerRadius::same(4), Color32::from_white_alpha(36));
        }
        cp.text(
            mute_r.center(),
            Align2::CENTER_CENTER,
            if st.muted { "M" } else { "♪" },
            semibold(10.5),
            if st.muted { Color32::from_gray(170) } else { Color32::WHITE },
        );
        if mute_resp.clicked() {
            p.toggle_video_mute(id, fi);
            ui.ctx().request_repaint();
        }
        let track_x0 = play_r.max.x + 6.0;
        let track_x1 = mute_r.min.x - 6.0;
        if track_x1 - track_x0 >= 28.0 {
            let secs = st.position.round().max(0.0) as u64;
            let time_txt = format!("{}:{:02}", secs / 60, secs % 60);
            let time_galley = cp.layout_no_wrap(time_txt, regular(9.5), Color32::from_gray(220));
            let bar_x1 = if track_x1 - track_x0 > time_galley.size().x + 36.0 {
                let tx = track_x1 - time_galley.size().x;
                cp.galley(pos2(tx, hud.center().y - time_galley.size().y / 2.0), time_galley, Color32::from_gray(220));
                tx - 6.0
            } else {
                track_x1
            };
            let hit = Rect::from_min_max(pos2(track_x0, hud.min.y + 4.0), pos2(bar_x1, hud.max.y - 4.0));
            let bar = Rect::from_min_max(pos2(hit.min.x, hud.center().y - 2.0), pos2(hit.max.x, hud.center().y + 2.0));
            cp.rect_filled(bar, CornerRadius::same(2), Color32::from_white_alpha(70));
            let prog = Rect::from_min_max(bar.min, pos2(bar.min.x + bar.width() * ratio, bar.max.y));
            cp.rect_filled(prog, CornerRadius::same(2), ACCENT);
            let scrub_resp = ui.interact(hit, ui.scope_id().with(("vid_scrub", id, fi)), Sense::click_and_drag());
            if (scrub_resp.clicked() || scrub_resp.dragged())
                && let Some(q) = scrub_resp.interact_pointer_pos()
                && bar.width() > 1.0
            {
                let r = f64::from((q.x - bar.min.x) / bar.width()).clamp(0.0, 1.0);
                p.seek_video(id, fi, r);
                ui.ctx().request_repaint();
            }
        }
    }
}
