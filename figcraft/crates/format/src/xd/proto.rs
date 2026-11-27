//! AGC prototype interaction mapping (`tap`, `drag`, `time`, `hover`, `keys`/`gamepad` triggers ->
//! `Navigate`, `OpenOverlay`, `Back`, `ScrollTo`, `OpenUrl` actions + Page `Flow` starting points).

use figcraft_doc::NodeId;
use figcraft_doc::NodeKind;
use figcraft_doc::proto::{Action, Direction, Easing, Flow, Interaction, MAX_DELAY_MS, OverlayAnchor, OverlayPosition, Transition, Trigger};
use serde_json::Value;

use super::style::{bool_at, f64_at, str_at};
use super::tree::ImportCtx;

pub(super) fn resolve_interactions(ctx: &mut ImportCtx<'_>, page_id: NodeId, home_frame: Option<NodeId>) {
    let pending = std::mem::take(&mut ctx.pending_interactions);
    let mut first_interactive_artboard: Option<NodeId> = None;

    for (node_id, arr_val) in pending {
        let Some(arr) = arr_val.as_array() else { continue };
        let mut built = Vec::new();
        for item in arr.iter().take(64) {
            if let Some(int) = map_interaction(ctx, item) {
                built.push(int);
            }
        }
        if !built.is_empty() {
            if first_interactive_artboard.is_none() {
                first_interactive_artboard = enclosing_artboard(&ctx.doc, page_id, node_id);
            }
            if let Some(n) = ctx.doc.node_mut(node_id) {
                n.interactions.extend(built);
            }
        }
    }

    let flow_start = home_frame.or(first_interactive_artboard);
    if let Some(start) = flow_start
        && let Some(NodeKind::Page { flows, .. }) = ctx.doc.node_mut(page_id).map(|n| &mut n.kind)
        && !flows.iter().any(|f| f.start == start)
    {
        flows.push(Flow { name: "Flow 1".into(), start, description: String::new() });
    }
}

fn enclosing_artboard(doc: &figcraft_doc::Document, page_id: NodeId, mut id: NodeId) -> Option<NodeId> {
    for _ in 0..256 {
        let n = doc.node(id)?;
        if n.parent == Some(page_id) && matches!(n.kind, NodeKind::Frame) {
            return Some(id);
        }
        id = n.parent?;
    }
    None
}

fn map_interaction(ctx: &mut ImportCtx<'_>, v: &Value) -> Option<Interaction> {
    if bool_at(v, "enabled") == Some(false) {
        return None;
    }
    let trig_obj = v.get("trigger").unwrap_or(v);
    let trig_str = trig_obj.as_str().or_else(|| str_at(trig_obj, "type")).or_else(|| str_at(v, "event")).unwrap_or("tap");

    let trigger = match trig_str.to_ascii_lowercase().as_str() {
        "tap" | "click" => Trigger::OnClick,
        "drag" => Trigger::OnDrag,
        "hover" | "rollover" => Trigger::OnHover,
        "press" => Trigger::OnPress,
        "mouseenter" | "mouse-enter" => Trigger::MouseEnter,
        "mouseleave" | "mouse-leave" => Trigger::MouseLeave,
        "time" | "delay" | "timeout" => {
            let props = v.get("properties").unwrap_or(trig_obj);
            let raw_delay = f64_at(props, "delay").or_else(|| f64_at(trig_obj, "delay")).or_else(|| f64_at(v, "delay")).unwrap_or(0.5);
            // Adobe XD stores `delay` in seconds (e.g. `0.8`), whereas > 60 without decimals may be ms.
            let ms = if raw_delay <= 60.0 {
                (raw_delay.max(0.0) * 1000.0).round() as u32
            } else {
                raw_delay.round().clamp(0.0, f64::from(MAX_DELAY_MS)) as u32
            };
            Trigger::AfterDelay { ms: ms.min(MAX_DELAY_MS) }
        }
        "keys" | "key" | "gamepad" => {
            let props = v.get("properties").unwrap_or(trig_obj);
            let key = str_at(props, "key").or_else(|| str_at(trig_obj, "key")).or_else(|| str_at(v, "key")).unwrap_or("Space").to_string();
            Trigger::KeyPress { key }
        }
        "voice" => {
            ctx.warn("Adobe XD voice prototype triggers are not supported and were skipped");
            return None;
        }
        other => {
            ctx.warn(format!("Unsupported Adobe XD prototype trigger `{other}` was skipped"));
            return None;
        }
    };

    let act_obj = v.get("action").unwrap_or(v);
    let props = v.get("properties").or_else(|| act_obj.get("properties")).unwrap_or(act_obj);
    let act_ty = act_obj.as_str().or_else(|| str_at(act_obj, "type")).or_else(|| str_at(props, "type")).unwrap_or("transition");

    let target_guid = str_at(props, "destination")
        .or_else(|| str_at(act_obj, "destination"))
        .or_else(|| str_at(v, "destination"))
        .or_else(|| str_at(props, "target"));

    let resolve_dest = |ctx: &mut ImportCtx<'_>, guid: Option<&str>| -> Option<NodeId> {
        let g = guid?.trim_start_matches("artboard-");
        ctx.artboard_map.get(guid?).or_else(|| ctx.artboard_map.get(g)).or_else(|| ctx.node_guid_map.get(guid?)).copied()
    };

    let preserve_scroll = bool_at(props, "preserveScrollPosition").or_else(|| bool_at(props, "preserveScroll")).unwrap_or(false);

    let action = match act_ty.to_ascii_lowercase().as_str() {
        "previousartboard" | "previous" | "back" => Action::Back,
        "overlay" => {
            let Some(dest) = resolve_dest(ctx, target_guid) else {
                ctx.warn("Prototype overlay interaction referencing missing artboard was skipped");
                return None;
            };
            let offset = match (f64_at(props, "overlayOffsetX"), f64_at(props, "overlayOffsetY")) {
                (Some(ox), Some(oy)) => Some([ox.clamp(-100_000.0, 100_000.0), oy.clamp(-100_000.0, 100_000.0)]),
                _ => None,
            };
            let position = if offset.is_some() { OverlayPosition::Manual } else { OverlayPosition::Center };
            Action::OpenOverlay { dest, position, close_on_outside: true, background: None, offset, anchor: OverlayAnchor::Screen }
        }
        "scrollto" | "scroll" => {
            let Some(dest) = resolve_dest(ctx, target_guid) else {
                ctx.warn("Prototype scrollTo interaction referencing missing layer was skipped");
                return None;
            };
            let oy = f64_at(props, "scrollOffset").or_else(|| f64_at(props, "offsetY")).unwrap_or(0.0);
            Action::ScrollTo { dest, offset: [0.0, oy.clamp(-100_000.0, 100_000.0)] }
        }
        "url" | "openurl" | "hyperlink" => {
            let Some(url) = str_at(props, "url").or_else(|| str_at(act_obj, "url")).filter(|u| !u.is_empty()) else {
                return None;
            };
            Action::OpenUrl { url: url.to_string(), new_tab: true }
        }
        "speech" | "audio" | "lottie" => {
            ctx.warn(format!("Adobe XD `{act_ty}` prototype action is not supported and was skipped"));
            return None;
        }
        _ => {
            let Some(dest) = resolve_dest(ctx, target_guid) else {
                ctx.warn("Prototype transition referencing missing artboard was skipped");
                return None;
            };
            Action::Navigate { dest, preserve_scroll }
        }
    };

    let direction = match str_at(props, "slideDirection").or_else(|| str_at(props, "direction")).unwrap_or("left").to_ascii_lowercase().as_str() {
        "right" => Direction::Right,
        "up" | "top" => Direction::Top,
        "down" | "bottom" => Direction::Bottom,
        _ => Direction::Left,
    };

    let trans_str = if act_ty.eq_ignore_ascii_case("autoanimate") {
        "autoAnimate"
    } else {
        str_at(props, "transition").or_else(|| str_at(act_obj, "transition")).unwrap_or("dissolve")
    };

    let transition = match trans_str.to_ascii_lowercase().as_str() {
        "none" | "instant" => Transition::Instant,
        "autoanimate" | "auto-animate" | "smartanimate" => Transition::SmartAnimate,
        "push" | "pushleft" | "push-left" => {
            Transition::Push { direction: if trans_str.to_ascii_lowercase().contains("right") { Direction::Right } else { direction } }
        }
        "slide" | "slidein" | "slide-left" | "slide-right" | "slide-up" | "slide-down" => Transition::SlideIn { direction },
        "movein" | "move-in" => Transition::MoveIn { direction },
        "moveout" | "move-out" => Transition::MoveOut { direction },
        _ => Transition::Dissolve,
    };

    let raw_dur = f64_at(props, "duration").unwrap_or(0.3);
    let duration_ms = if raw_dur <= 30.0 { (raw_dur.max(0.0) * 1000.0).round() as u32 } else { raw_dur.round().clamp(0.0, 10_000.0) as u32 };

    let easing = match str_at(props, "easing").unwrap_or("ease-out").to_ascii_lowercase().as_str() {
        "linear" | "none" => Easing::Linear,
        "ease-in" | "easein" => Easing::EaseIn,
        "ease-in-out" | "easeinout" => Easing::EaseInOut,
        "wind-up" | "ease-in-back" => Easing::EaseInBack,
        "snap" | "ease-out-back" => Easing::EaseOutBack,
        "bounce" | "bouncy" => Easing::Bouncy,
        _ => Easing::EaseOut,
    };

    Some(Interaction { trigger, actions: vec![action], transition, duration_ms, easing, delay_ms: 0 })
}
