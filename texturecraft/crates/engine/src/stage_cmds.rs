//! 3D Stage & Studio Lighting commands: multi-light studio rig presets, auto-align to ground plane,
//! ground-plane shadow catcher, and camera focal-length / depth-of-field bookmarks.

use serde_json::{Value, json};
use texturecraft_core::{Camera, DepthOfField, StudioRig, focal_mm_to_fov_deg, fov_deg_to_focal_mm};

use crate::commands::{CommandSpec, always, bad, boolp, f32p, strp, u64p};
use crate::{Result, Session, cmd};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "stage.set",
            "Stage scene settings",
            [],
            None,
            "{rig?: none|productThreePoint|packshotRim|tabletopWarm|dramaticLowKey, rigIntensity?: 0..4, rigRotation?: -360..360, shadowCatcher?: bool, groundY?, groundAuto?: bool, shadowOpacity?: 0..1, contactAo?: 0..1, contactAoRadius?: 0.1..5, lightSoftness?: 0..1, focalLengthMm?: 8..300}",
            always,
            stage_set
        ),
        cmd!("stage.snapToGround", "Snap model to ground plane", ["Viewport"], None, "{groundY?: 0.0, enableShadowCatcher?: bool}", always, snap_to_ground),
        cmd!(
            "stage.saveCameraBookmark",
            "Save camera and DoF bookmark",
            [],
            None,
            "{name?, focalLengthMm?, fov?, dofEnabled?, focusDistance?, aperture?, replace?: bool}",
            always,
            save_camera_bookmark
        ),
        cmd!("stage.applyCameraBookmark", "Apply camera and DoF bookmark", [], None, "{index? | name?}", has_bookmarks, apply_camera_bookmark),
    ]
}

fn has_bookmarks(s: &Session) -> std::result::Result<(), String> {
    if s.doc.project.cameras.is_empty() { Err("the project has no saved camera bookmarks".into()) } else { Ok(()) }
}

fn stage_set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "stage.set";
    let rig = match strp(p, "rig") {
        Some(name) => Some(StudioRig::parse(name).ok_or_else(|| bad(CMD, format!("unknown studio rig `{name}`")))?),
        None => None,
    };
    let focal_mm = f32p(p, "focalLengthMm").map(|mm| mm.clamp(8.0, 300.0));

    s.edit("Stage settings", |s| {
        let d = &mut s.doc.project.display;
        if let Some(rig) = rig {
            d.stage.rig = rig;
            if let Some(env_name) = rig.environment_name() {
                d.environment = env_name.to_string();
                d.shadows = true;
            }
        }
        if let Some(v) = f32p(p, "rigIntensity") {
            d.stage.rig_intensity = v.clamp(0.0, 4.0);
        }
        if let Some(v) = f32p(p, "rigRotation") {
            d.stage.rig_rotation = v.clamp(-360.0, 360.0);
        }
        if let Some(b) = boolp(p, "shadowCatcher") {
            d.stage.shadow_catcher = b;
        }
        if let Some(v) = f32p(p, "groundY") {
            d.stage.ground_y = v.clamp(-1e5, 1e5);
            d.stage.ground_auto = false;
        }
        if let Some(b) = boolp(p, "groundAuto") {
            d.stage.ground_auto = b;
        }
        if let Some(v) = f32p(p, "shadowOpacity") {
            d.stage.shadow_opacity = v.clamp(0.0, 1.0);
        }
        if let Some(v) = f32p(p, "contactAo") {
            d.stage.contact_ao = v.clamp(0.0, 1.0);
        }
        if let Some(v) = f32p(p, "contactAoRadius") {
            d.stage.contact_ao_radius = v.clamp(0.1, 5.0);
        }
        if let Some(v) = f32p(p, "lightSoftness") {
            d.stage.light_softness = v.clamp(0.0, 1.0);
        }
        if let Some(mm) = focal_mm {
            let fov = focal_mm_to_fov_deg(mm);
            d.fov = fov;
            s.camera.fov = fov;
        }
        Ok(serde_json::to_value(&s.doc.project.display.stage).unwrap_or(Value::Null))
    })
}

fn snap_to_ground(s: &mut Session, p: &Value) -> Result<Value> {
    let ground_y = f32p(p, "groundY").unwrap_or(0.0).clamp(-1e5, 1e5);
    let enable_catcher = boolp(p, "enableShadowCatcher").unwrap_or(true);
    let (src_lo, _) = s.source.bounds();
    let new_offset_y = ground_y - src_lo.y;
    let prev_offset_y = s.doc.project.display.stage.model_offset_y;
    let delta_y = new_offset_y - prev_offset_y;

    s.edit("Snap to ground", |s| {
        let stage = &mut s.doc.project.display.stage;
        stage.ground_y = ground_y;
        stage.ground_auto = false;
        stage.model_offset_y = new_offset_y;
        if enable_catcher {
            stage.shadow_catcher = true;
        }
        Ok(())
    })?;

    if delta_y.is_finite() {
        s.camera.target.y += delta_y;
    }
    let (lo, hi) = s.mesh.bounds();
    Ok(json!({
        "groundY": ground_y,
        "offsetY": new_offset_y,
        "boundsMin": lo.to_array(),
        "boundsMax": hi.to_array(),
    }))
}

fn save_camera_bookmark(s: &mut Session, p: &Value) -> Result<Value> {
    let base_name = strp(p, "name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Stage Camera");
    let replace = boolp(p, "replace").unwrap_or(false);

    let fov = if let Some(mm) = f32p(p, "focalLengthMm") {
        focal_mm_to_fov_deg(mm.clamp(8.0, 300.0))
    } else if let Some(fov_deg) = f32p(p, "fov") {
        fov_deg.clamp(5.0, 150.0)
    } else {
        s.camera.fov.clamp(5.0, 150.0)
    };
    let focal_mm = f32p(p, "focalLengthMm").map(|mm| mm.clamp(8.0, 300.0)).unwrap_or_else(|| fov_deg_to_focal_mm(fov));

    let cur_dof = s.doc.project.display.dof;
    let dof = DepthOfField {
        enabled: boolp(p, "dofEnabled").unwrap_or(cur_dof.enabled),
        focus_distance: f32p(p, "focusDistance").map(|d| d.clamp(0.0, 1e6)).unwrap_or(cur_dof.focus_distance),
        aperture: f32p(p, "aperture").map(|a| a.clamp(0.0, 1.0)).unwrap_or(cur_dof.aperture),
    };

    let existing_idx = if replace { s.doc.project.cameras.iter().position(|c| c.name == base_name) } else { None };
    let final_name = if existing_idx.is_some() {
        base_name.to_string()
    } else {
        let taken = |n: &str| n == crate::camera_cmds::DEFAULT_CAMERA || s.doc.project.cameras.iter().any(|c| c.name == n);
        if !taken(base_name) {
            base_name.to_string()
        } else {
            (2..).map(|i| format!("{base_name} {i}")).find(|n| !taken(n)).unwrap_or_else(|| base_name.to_string())
        }
    };

    let cam = Camera {
        name: final_name.clone(),
        target: s.camera.target.to_array(),
        yaw: s.camera.yaw,
        pitch: s.camera.pitch,
        distance: s.camera.distance,
        fov,
        ortho: s.camera.ortho,
        focal_length_mm: Some(focal_mm),
        dof: Some(dof),
    };

    s.edit("Save camera bookmark", |s| {
        s.camera.fov = fov;
        s.doc.project.display.fov = fov;
        s.doc.project.display.dof = dof;
        let idx = match existing_idx {
            Some(i) if i < s.doc.project.cameras.len() => {
                s.doc.project.cameras[i] = cam;
                i
            }
            _ => {
                s.doc.project.cameras.push(cam);
                s.doc.project.cameras.len() - 1
            }
        };
        Ok(json!({
            "index": idx,
            "name": final_name,
            "fov": fov,
            "focalLengthMm": focal_mm,
            "dof": dof,
        }))
    })
}

fn apply_camera_bookmark(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "stage.applyCameraBookmark";
    let cams = &s.doc.project.cameras;
    let idx = match (u64p(p, "index"), strp(p, "name")) {
        (Some(i), _) => usize::try_from(i).ok().filter(|i| *i < cams.len()),
        (None, Some(name)) => cams.iter().position(|c| c.name == name),
        (None, None) => (!cams.is_empty()).then_some(0),
    }
    .ok_or_else(|| bad(CMD, "no such camera bookmark"))?;

    let bookmark = cams.get(idx).cloned().ok_or_else(|| bad(CMD, "no such camera bookmark"))?;
    let resolved_fov = bookmark.focal_length_mm.filter(|mm| mm.is_finite()).map(focal_mm_to_fov_deg).unwrap_or_else(|| bookmark.fov.clamp(5.0, 150.0));
    let resolved_mm = bookmark.focal_length_mm.unwrap_or_else(|| fov_deg_to_focal_mm(resolved_fov));

    s.edit("Apply camera bookmark", |s| {
        crate::camera_cmds::apply_saved_camera(&mut s.camera, &bookmark);
        s.camera.fov = resolved_fov;
        s.doc.project.display.fov = resolved_fov;
        if let Some(dof) = bookmark.dof {
            s.doc.project.display.dof = dof;
        }
        Ok(json!({
            "index": idx,
            "name": bookmark.name,
            "fov": resolved_fov,
            "focalLengthMm": resolved_mm,
            "dof": s.doc.project.display.dof,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execute;

    fn session() -> Session {
        Session::new_project("stage_test", texturecraft_geom::primitives::sphere(1.0, 24, 12), "builtin:sphere", None, 128).unwrap()
    }

    #[test]
    fn snap_to_ground_aligns_mesh_min_y_and_supports_undo_and_save() {
        let mut s = session();
        assert!((s.mesh.bounds().0.y - (-1.0)).abs() < 1e-4);

        let res = execute(&mut s, "stage.snapToGround", &json!({"groundY": 0.25})).unwrap();
        assert!((res["boundsMin"][1].as_f64().unwrap() as f32 - 0.25).abs() < 1e-4);
        assert!((s.mesh.bounds().0.y - 0.25).abs() < 1e-4);
        assert!(s.doc.project.display.stage.shadow_catcher);

        // Round-trips through project bytes
        let bytes = texturecraft_formats::save_project_bytes(&s.project_file()).unwrap();
        let reopened = Session::from_project_bytes(&bytes).unwrap();
        assert!((reopened.mesh.bounds().0.y - 0.25).abs() < 1e-4);

        // Undo restores original mesh bounds
        execute(&mut s, "edit.undo", &Value::Null).unwrap();
        assert!((s.mesh.bounds().0.y - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn studio_rigs_and_camera_dof_bookmarks_round_trip() {
        let mut s = session();
        execute(&mut s, "stage.set", &json!({"rig": "productThreePoint", "rigIntensity": 1.4, "shadowCatcher": true})).unwrap();
        assert_eq!(s.doc.project.display.stage.rig, StudioRig::ProductThreePoint);
        assert_eq!(s.doc.project.display.environment, "Product Three-Point");

        execute(&mut s, "camera.set", &json!({"yaw": 45.0, "pitch": 20.0, "distance": 4.0})).unwrap();
        let saved = execute(
            &mut s,
            "stage.saveCameraBookmark",
            &json!({"name": "Hero 85mm", "focalLengthMm": 85.0, "dofEnabled": true, "focusDistance": 3.2, "aperture": 0.18}),
        )
        .unwrap();
        let hero_fov = saved["fov"].as_f64().unwrap() as f32;
        assert!((hero_fov - focal_mm_to_fov_deg(85.0)).abs() < 1e-3);

        // Move camera and change lens/DoF away from bookmark
        execute(&mut s, "camera.set", &json!({"yaw": -30.0, "fov": 60.0})).unwrap();
        execute(&mut s, "display.set", &json!({"dof": {"enabled": false, "focusDistance": 1.0, "aperture": 0.02}})).unwrap();

        // Apply bookmark restores orbit, focal length FOV, and DoF
        let applied = execute(&mut s, "stage.applyCameraBookmark", &json!({"name": "Hero 85mm"})).unwrap();
        assert!((applied["focalLengthMm"].as_f64().unwrap() as f32 - 85.0).abs() < 1e-3);
        assert!((s.camera.fov - hero_fov).abs() < 1e-3);
        assert!(s.doc.project.display.dof.enabled);
        assert!((s.doc.project.display.dof.focus_distance - 3.2).abs() < 1e-4);
        assert!((s.doc.project.display.dof.aperture - 0.18).abs() < 1e-4);
    }
}
