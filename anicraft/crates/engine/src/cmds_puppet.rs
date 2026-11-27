//! Puppet rigging and procedural character animation commands (`puppet.*`):
//! - `puppet.configure_head_turn` / `puppet.pose_head`: 5-pose head-turn parallax rig
//! - `puppet.define_swap_set` / `puppet.trigger_swap`: `Hold` & `Latch` swap-set keyframe stamping
//! - `puppet.auto_blink`, `puppet.breathe`, `puppet.walk_cycle`: procedural character behaviors
//! - `puppet.info`: inspect document puppet rigs

use crate::params::*;
use crate::{CommandSpec, EngineError, MAX_FRAME, Result, Session};
use anicraft_doc::puppet::{
    AutoBlinkConfig, BreatheConfig, FacialFeature, FeatureParallax, HeadPose, PoseOffset, PuppetRig, SwapChoice, SwapSet, TriggerEvent, TriggerMode,
    WalkCycleConfig, WalkRigLayers, apply_auto_blink, apply_breathe, apply_walk_cycle,
};
use anicraft_doc::{Keyframe, LayerId, LayerKind, SymbolId};
use anicraft_geom::Vec2;
use serde_json::{Value, json};
use std::sync::Arc;

pub fn specs() -> Vec<CommandSpec> {
    let c = |id, label, menu, shortcut, params, mutates, run| CommandSpec { id, label, menu, shortcut, params, mutates, run };
    vec![
        c(
            "puppet.configure_head_turn",
            "Head-Turn Rig",
            "",
            None,
            "{id?, name?, pivot?: [x, y], frontal?, left_quarter?, right_quarter?, upward?, downward?, features?: [{name?, layer?, feature: nose|left_eye|right_eye|mouth|left_brow|right_brow|left_ear|right_ear|hair_front|hair_back|head_contour, weight?}]} creates or updates a 5-pose head-turn parallax rig",
            true,
            configure_head_turn as crate::RunFn,
        ),
        c(
            "puppet.pose_head",
            "Pose Head Turn",
            "",
            None,
            "{rig? (id, default: first puppet rig), yaw?: -1..1, pitch?: -1..1, pose?: frontal|left_quarter|right_quarter|upward|downward, frame?} evaluates feature parallax weights and keys the bound layers at the frame",
            true,
            pose_head,
        ),
        c(
            "puppet.define_swap_set",
            "Define Swap Set",
            "",
            None,
            "{rig?, name, layer?, mode?: hold|latch, default_choice?: index, choices: [{name, key, frame?, symbol?}]} defines or updates a swap-set trigger group",
            true,
            define_swap_set,
        ),
        c(
            "puppet.trigger_swap",
            "Trigger Swap Set",
            "",
            None,
            "{rig?, swap_set: name, layer?, frames?, events: [{frame, duration?, trigger}]} stamps SingleFrame symbol keyframes across the timeline using Hold or Latch semantics",
            true,
            trigger_swap,
        ),
        c(
            "puppet.auto_blink",
            "Auto Blink",
            "Modify/Puppet",
            None,
            "{layer?, interval?: frames (default 72), duration?: frames (default 3), jitter?: 0..0.8 (default 0.25), open_frame?: 0, closed_frame?: 1, seed?, from?, to?} stamps periodic blink keyframes on the layer",
            true,
            auto_blink,
        ),
        c(
            "puppet.breathe",
            "Breathe Cycle",
            "Modify/Puppet",
            None,
            "{layer?, period?: frames (default 72), scale_x?: 1.03, scale_y?: 1.05, lift?: px (default 4), pivot?: [x, y], phase?: deg, from?, to?} stamps rhythmic chest/torso breathing keyframes around the pivot",
            true,
            breathe,
        ),
        c(
            "puppet.walk_cycle",
            "Walk Cycle",
            "Modify/Puppet",
            None,
            "{cycle_frames?: 24, stride_length?: 48, step_height?: 22, hip_bob?: 6, arm_swing?: 28, torso_lean?: 3.5, speed?: 0, hip_layer?, left_leg?, right_leg?, left_arm?, right_arm?, from?, to?} generates procedural locomotion IK/transform keyframes",
            true,
            walk_cycle,
        ),
        c(
            "puppet.info",
            "Puppet Rig Info",
            "",
            None,
            "{} -> {puppets: [...]} lists puppet rigs, head-turn poses, and swap sets in the document",
            false,
            info,
        ),
    ]
}

fn parse_vec2(v: &Value) -> Option<Vec2> {
    let arr = v.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let x = arr.first()?.as_f64()? as f32;
    let y = arr.get(1)?.as_f64()? as f32;
    (x.is_finite() && y.is_finite()).then_some(Vec2::new(x, y))
}

fn parse_pose_offset(v: &Value, base: PoseOffset) -> Result<PoseOffset> {
    if !v.is_object() {
        return Err(EngineError::Param("pose offset must be an object {dx?, dy?, sx?, sy?, rotation?}".into()));
    }
    let dx = opt_f32(v, "dx")?.or(opt_f32(v, "x")?).unwrap_or(base.offset.x);
    let dy = opt_f32(v, "dy")?.or(opt_f32(v, "y")?).unwrap_or(base.offset.y);
    let sx = opt_f32(v, "sx")?.unwrap_or(base.scale.x);
    let sy = opt_f32(v, "sy")?.unwrap_or(base.scale.y);
    let rot = opt_f32(v, "rotation")?.unwrap_or(base.rotation_deg);
    Ok(PoseOffset { offset: Vec2::new(dx, dy), scale: Vec2::new(sx, sy), rotation_deg: rot }.sanitized())
}

fn configure_head_turn(s: &mut Session, p: &Value) -> Result<Value> {
    let req_id = opt_u64(p, "id")?;
    let name = opt_str(p, "name")?.map(|n| n.trim().chars().take(128).collect::<String>()).filter(|n| !n.is_empty());
    let pivot = match p.get("pivot").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(parse_vec2(v).ok_or_else(|| EngineError::Param("param 'pivot' must be [x, y]".into()))?),
    };

    let mut parsed_features: Option<Vec<FeatureParallax>> = None;
    if let Some(list) = p.get("features").filter(|v| !v.is_null()) {
        let arr = list.as_array().ok_or_else(|| EngineError::Param("param 'features' must be an array".into()))?;
        if arr.len() > anicraft_doc::puppet::MAX_FEATURE_BINDINGS {
            return Err(EngineError::Param("too many feature bindings".into()));
        }
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            let f_str = str_req(item, "feature")?;
            let feature = FacialFeature::from_key(f_str).ok_or_else(|| EngineError::Param(format!("unknown facial feature '{f_str}'")))?;
            let layer = opt_u64(item, "layer")?.map(LayerId);
            if let Some(lid) = layer
                && s.timeline().layer(lid).is_none()
            {
                return Err(EngineError::State(format!("no layer with id {}", lid.0)));
            }
            let fname = opt_str(item, "name")?.map(str::to_string).unwrap_or_else(|| feature.key().to_string());
            let mut binding = FeatureParallax::new(fname, layer, feature);
            if let Some(w) = opt_f32(item, "weight")? {
                binding.weight = w;
            }
            out.push(binding.sanitized());
        }
        parsed_features = Some(out);
    }

    let mut pose_edits = Vec::new();
    for hp in HeadPose::ALL {
        if let Some(sub) = p.get(hp.key()).filter(|v| !v.is_null()) {
            pose_edits.push((hp, sub.clone()));
        }
    }

    let existing_rig_id = match req_id {
        Some(id) => s.doc.puppets.iter().find(|r| r.id == id).map(|r| r.id),
        None if s.doc.puppets.len() == 1 => Some(s.doc.puppets[0].id),
        None => None,
    };
    let rig_id = match existing_rig_id {
        Some(id) => id,
        None => {
            if s.doc.puppets.len() >= anicraft_doc::puppet::MAX_PUPPET_RIGS {
                return Err(EngineError::State("maximum puppet rigs reached".into()));
            }
            let fresh = s.doc.alloc_id();
            Arc::make_mut(&mut s.doc.puppets).push(PuppetRig::new(fresh, name.clone().unwrap_or_else(|| "Puppet".into())));
            fresh
        }
    };
    let puppets = Arc::make_mut(&mut s.doc.puppets);
    let rig = puppets.iter_mut().find(|r| r.id == rig_id).ok_or_else(|| EngineError::State("puppet rig missing".into()))?;
    if let Some(n) = name {
        rig.name = n;
    }
    if let Some(pv) = pivot {
        rig.head_turn.pivot = pv;
    }
    for (hp, val) in pose_edits {
        let base = rig.head_turn.pose(hp);
        let updated = parse_pose_offset(&val, base)?;
        rig.head_turn.set_pose(hp, updated);
    }
    if let Some(feats) = parsed_features {
        rig.head_turn.features = feats;
    }
    *rig = rig.sanitized();

    Ok(json!({
        "rig": rig.id,
        "name": rig.name,
        "features": rig.head_turn.features.len(),
    }))
}

fn pose_head(s: &mut Session, p: &Value) -> Result<Value> {
    let rig_id = opt_u64(p, "rig")?;
    let frame = opt_u32(p, "frame")?.unwrap_or(s.frame);
    if frame > MAX_FRAME {
        return Err(EngineError::Param(format!("param 'frame' must be at most {MAX_FRAME}")));
    }
    let (preset_yaw, preset_pitch) = match opt_str(p, "pose")? {
        None => (0.0, 0.0),
        Some(k) => match HeadPose::from_key(k) {
            Some(HeadPose::Frontal) => (0.0, 0.0),
            Some(HeadPose::LeftQuarter) => (-1.0, 0.0),
            Some(HeadPose::RightQuarter) => (1.0, 0.0),
            Some(HeadPose::Upward) => (0.0, 1.0),
            Some(HeadPose::Downward) => (0.0, -1.0),
            None => return Err(EngineError::Param(format!("unknown head pose '{k}'"))),
        },
    };
    let yaw = opt_f32(p, "yaw")?.unwrap_or(preset_yaw).clamp(-1.0, 1.0);
    let pitch = opt_f32(p, "pitch")?.unwrap_or(preset_pitch).clamp(-1.0, 1.0);

    let rig = match rig_id {
        Some(id) => s.doc.puppets.iter().find(|r| r.id == id).cloned(),
        None => s.doc.puppets.first().cloned(),
    }
    .ok_or_else(|| EngineError::State("no puppet rig configured in this document (call puppet.configure_head_turn first)".into()))?;

    let pose = rig.head_turn.evaluate_pose(yaw, pitch);
    let tl = crate::editing::edited_mut(&mut s.doc, s.scene, &s.editing)?;
    let updated = rig.head_turn.apply_at_frame(tl, frame, yaw, pitch);
    Ok(json!({
        "rig": rig.id,
        "frame": frame,
        "yaw": yaw,
        "pitch": pitch,
        "offset": [pose.offset.x, pose.offset.y],
        "scale": [pose.scale.x, pose.scale.y],
        "rotation": pose.rotation_deg,
        "layers_updated": updated,
    }))
}

fn define_swap_set(s: &mut Session, p: &Value) -> Result<Value> {
    let rig_id = opt_u64(p, "rig")?;
    let name: String = str_req(p, "name")?.trim().chars().take(128).collect();
    if name.is_empty() {
        return Err(EngineError::Param("param 'name' must not be empty".into()));
    }
    let layer = opt_u64(p, "layer")?.map(LayerId);
    if let Some(lid) = layer
        && s.timeline().layer(lid).is_none()
    {
        return Err(EngineError::State(format!("no layer with id {}", lid.0)));
    }
    let mode = match opt_str(p, "mode")? {
        None => TriggerMode::Hold,
        Some(m) => TriggerMode::from_key(m).ok_or_else(|| EngineError::Param(format!("unknown trigger mode '{m}' (hold or latch)")))?,
    };
    let default_choice = opt_u32(p, "default_choice")?.unwrap_or(0) as usize;
    let choices_val = p
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| EngineError::Param("param 'choices' (array of {name, key, frame?, symbol?}) is required".into()))?;
    if choices_val.is_empty() || choices_val.len() > anicraft_doc::puppet::MAX_SWAP_CHOICES {
        return Err(EngineError::Param("param 'choices' must have between 1 and 128 entries".into()));
    }
    let mut choices = Vec::with_capacity(choices_val.len());
    for (idx, item) in choices_val.iter().enumerate() {
        let cname = str_req(item, "name")?.trim().to_string();
        let key = opt_str(item, "key")?.unwrap_or("").trim().to_string();
        let cframe = opt_u32(item, "frame")?.unwrap_or(idx as u32).min(MAX_FRAME);
        let symbol = opt_u64(item, "symbol")?.map(SymbolId);
        choices.push(SwapChoice { name: cname, key, frame: cframe, symbol }.sanitized());
    }

    let swap = SwapSet { name: name.clone(), layer, mode, default_choice, choices }.sanitized();

    if s.doc.puppets.is_empty() {
        if rig_id.is_some() {
            return Err(EngineError::State("requested puppet rig does not exist".into()));
        }
        let fresh = s.doc.alloc_id();
        Arc::make_mut(&mut s.doc.puppets).push(PuppetRig::new(fresh, "Puppet"));
    }
    let puppets = Arc::make_mut(&mut s.doc.puppets);
    let rig = match rig_id {
        Some(id) => puppets.iter_mut().find(|r| r.id == id).ok_or_else(|| EngineError::State(format!("no puppet rig with id {id}")))?,
        None => puppets.first_mut().ok_or_else(|| EngineError::State("no puppet rig".into()))?,
    };
    if let Some(existing) = rig.swap_sets.iter_mut().find(|ss| ss.name.eq_ignore_ascii_case(&name)) {
        *existing = swap.clone();
    } else {
        if rig.swap_sets.len() >= anicraft_doc::puppet::MAX_SWAP_SETS {
            return Err(EngineError::State("maximum swap sets reached".into()));
        }
        rig.swap_sets.push(swap.clone());
    }

    Ok(json!({
        "rig": rig.id,
        "swap_set": swap.name,
        "mode": swap.mode.key(),
        "choices": swap.choices.len(),
    }))
}

fn trigger_swap(s: &mut Session, p: &Value) -> Result<Value> {
    let rig_id = opt_u64(p, "rig")?;
    let set_name = str_req(p, "swap_set")?;
    let rig = match rig_id {
        Some(id) => s.doc.puppets.iter().find(|r| r.id == id),
        None => s.doc.puppets.first(),
    }
    .ok_or_else(|| EngineError::State("no puppet rig configured".into()))?;
    let swap = rig
        .swap_sets
        .iter()
        .find(|ss| ss.name.eq_ignore_ascii_case(set_name))
        .cloned()
        .ok_or_else(|| EngineError::State(format!("no swap set named '{set_name}'")))?;

    let target_layer = opt_u64(p, "layer")?.map(LayerId).or(swap.layer).unwrap_or(s.layer);
    let events_arr = p
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| EngineError::Param("param 'events' (array of {frame, duration?, trigger}) is required".into()))?;
    if events_arr.len() > 10_000 {
        return Err(EngineError::Param("too many trigger events".into()));
    }
    let mut events = Vec::with_capacity(events_arr.len());
    let mut max_ev_end = 1u32;
    for ev in events_arr {
        let frame = opt_u32(ev, "frame")?.ok_or_else(|| EngineError::Param("param 'frame' is required".into()))?.min(MAX_FRAME);
        let duration = opt_u32(ev, "duration")?.unwrap_or(1).clamp(1, MAX_FRAME);
        let trigger = str_req(ev, "trigger")?.to_string();
        max_ev_end = max_ev_end.max(frame.saturating_add(duration).min(MAX_FRAME));
        events.push(TriggerEvent { frame, duration, trigger });
    }
    let total_frames = opt_u32(p, "frames")?.unwrap_or_else(|| s.frame_count().max(max_ev_end).max(24)).clamp(1, MAX_FRAME);

    let layer = s.layer_mut(Some(target_layer))?;
    if matches!(layer.kind, LayerKind::Folder | LayerKind::Camera) {
        return Err(EngineError::State("cannot stamp swap keyframes onto a folder or camera layer".into()));
    }
    let stamped = swap.stamp_triggers(layer, &events, total_frames);
    if stamped == 0 {
        return Err(EngineError::State("target layer has no symbol or artwork on its first keyframe to swap".into()));
    }
    Ok(json!({
        "layer": target_layer.0,
        "swap_set": swap.name,
        "mode": swap.mode.key(),
        "keyframes": stamped,
    }))
}

const MAX_BEHAVIOR_SPAN: u32 = 4_800;

fn span_range(s: &Session, p: &Value, default_min_len: u32) -> Result<(u32, u32)> {
    let from = opt_u32(p, "from")?.unwrap_or(0).min(MAX_FRAME);
    let timeline_end = s.frame_count().max(from.saturating_add(default_min_len)).min(from.saturating_add(MAX_BEHAVIOR_SPAN)).min(MAX_FRAME);
    let to = opt_u32(p, "to")?.unwrap_or(timeline_end).min(MAX_FRAME);
    if to <= from {
        return Err(EngineError::Param("param 'to' must be greater than 'from'".into()));
    }
    if to - from > MAX_BEHAVIOR_SPAN {
        return Err(EngineError::Param(format!("procedural behavior span ({}) exceeds maximum of {MAX_BEHAVIOR_SPAN} frames", to - from)));
    }
    Ok((from, to))
}

fn auto_blink(s: &mut Session, p: &Value) -> Result<Value> {
    let lid = opt_u64(p, "layer")?.map(LayerId).unwrap_or(s.layer);
    let interval = opt_u32(p, "interval")?.unwrap_or(72).clamp(4, 10_000);
    let duration = opt_u32(p, "duration")?.unwrap_or(3).clamp(1, 240);
    let jitter = opt_f32(p, "jitter")?.unwrap_or(0.25).clamp(0.0, 0.8);
    let open_frame = opt_u32(p, "open_frame")?.unwrap_or(0).min(MAX_FRAME);
    let closed_frame = opt_u32(p, "closed_frame")?.unwrap_or(1).min(MAX_FRAME);
    let seed = opt_u64(p, "seed")?.unwrap_or(0xA11C_EB11);
    let (from, to) = span_range(s, p, interval.max(48))?;

    let layer = s.layer_mut(Some(lid))?;
    if matches!(layer.kind, LayerKind::Folder | LayerKind::Camera) {
        return Err(EngineError::State("Auto Blink requires an artwork layer".into()));
    }
    let cfg = AutoBlinkConfig { interval_frames: interval, duration_frames: duration, jitter, open_frame, closed_frame, seed };
    let stamped = apply_auto_blink(layer, &cfg, from, to);
    if stamped == 0 {
        return Err(EngineError::State("the layer has no artwork to blink".into()));
    }
    Ok(json!({
        "layer": lid.0,
        "keyframes": stamped,
        "from": from,
        "to": to,
    }))
}

fn breathe(s: &mut Session, p: &Value) -> Result<Value> {
    let lid = opt_u64(p, "layer")?.map(LayerId).unwrap_or(s.layer);
    let period = opt_u32(p, "period")?.unwrap_or(72).clamp(8, 10_000);
    let scale_x = opt_f32(p, "scale_x")?.unwrap_or(1.03).clamp(0.5, 2.0);
    let scale_y = opt_f32(p, "scale_y")?.unwrap_or(1.05).clamp(0.5, 2.0);
    let lift_px = opt_f32(p, "lift")?.unwrap_or(4.0).clamp(-200.0, 200.0);
    let phase_deg = opt_f32(p, "phase")?.unwrap_or(0.0).clamp(-3600.0, 3600.0);
    let pivot = match p.get("pivot").filter(|v| !v.is_null()) {
        None => None,
        Some(v) => Some(parse_vec2(v).ok_or_else(|| EngineError::Param("param 'pivot' must be [x, y]".into()))?),
    };
    let (from, to) = span_range(s, p, period)?;

    let layer = s.layer_mut(Some(lid))?;
    if matches!(layer.kind, LayerKind::Folder | LayerKind::Camera) {
        return Err(EngineError::State("Breathe Cycle requires an artwork layer".into()));
    }
    let cfg = BreatheConfig { period_frames: period, scale_x, scale_y, lift_px, pivot, phase_deg };
    let stamped = apply_breathe(layer, &cfg, from, to);
    if stamped == 0 {
        return Err(EngineError::State("the layer has no artwork to animate".into()));
    }
    Ok(json!({
        "layer": lid.0,
        "keyframes": stamped,
        "from": from,
        "to": to,
    }))
}

fn walk_cycle(s: &mut Session, p: &Value) -> Result<Value> {
    let cycle_frames = opt_u32(p, "cycle_frames")?.unwrap_or(24).clamp(4, 10_000);
    let stride_length = opt_f32(p, "stride_length")?.unwrap_or(48.0).clamp(-1000.0, 1000.0);
    let step_height = opt_f32(p, "step_height")?.unwrap_or(22.0).clamp(0.0, 500.0);
    let hip_bob = opt_f32(p, "hip_bob")?.unwrap_or(6.0).clamp(0.0, 200.0);
    let arm_swing_deg = opt_f32(p, "arm_swing")?.unwrap_or(28.0).clamp(-180.0, 180.0);
    let torso_lean_deg = opt_f32(p, "torso_lean")?.unwrap_or(3.5).clamp(-60.0, 60.0);
    let root_speed_per_cycle = opt_f32(p, "speed")?.unwrap_or(0.0).clamp(-2000.0, 2000.0);
    let (from, to) = span_range(s, p, cycle_frames)?;

    let rig = WalkRigLayers {
        hip: opt_u64(p, "hip_layer")?.map(LayerId),
        left_leg: opt_u64(p, "left_leg")?.map(LayerId),
        right_leg: opt_u64(p, "right_leg")?.map(LayerId),
        left_arm: opt_u64(p, "left_arm")?.map(LayerId),
        right_arm: opt_u64(p, "right_arm")?.map(LayerId),
    };
    for lid in [rig.hip, rig.left_leg, rig.right_leg, rig.left_arm, rig.right_arm].into_iter().flatten() {
        if s.timeline().layer(lid).is_none() {
            return Err(EngineError::State(format!("no layer with id {}", lid.0)));
        }
    }

    let fallback = s.layer;
    let cfg = WalkCycleConfig { cycle_frames, stride_length, step_height, hip_bob, arm_swing_deg, torso_lean_deg, root_speed_per_cycle };
    let tl = crate::editing::edited_mut(&mut s.doc, s.scene, &s.editing)?;
    let (layers_updated, keyframes) = apply_walk_cycle(tl, &rig, fallback, &cfg, from, to);
    if keyframes == 0 {
        // Ensure empty single-keyframe check returns clean state error rather than mutating nothing silently.
        let _ = Keyframe::new(0, 1);
        return Err(EngineError::State("no artwork found on the target walk-cycle layers".into()));
    }
    Ok(json!({
        "layers": layers_updated,
        "keyframes": keyframes,
        "from": from,
        "to": to,
    }))
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    let list: Vec<Value> = s
        .doc
        .puppets
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "name": r.name,
                "pivot": [r.head_turn.pivot.x, r.head_turn.pivot.y],
                "features": r.head_turn.features.iter().map(|f| json!({
                    "name": f.name,
                    "layer": f.layer.map(|l| l.0),
                    "feature": f.feature.key(),
                    "weight": f.weight,
                })).collect::<Vec<_>>(),
                "swap_sets": r.swap_sets.iter().map(|ss| json!({
                    "name": ss.name,
                    "layer": ss.layer.map(|l| l.0),
                    "mode": ss.mode.key(),
                    "default_choice": ss.default_choice,
                    "choices": ss.choices.len(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({ "puppets": list }))
}
