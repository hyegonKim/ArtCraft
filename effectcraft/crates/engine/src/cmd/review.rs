//! Frame.io (`FFC_framemob`) Team Review & Approval Workflow commands for compositions:
//! - `review.importFrameIo`: import Frame.io JSON or CSV comments + Telestrator annotations into
//!   the active (or named `comp`) composition's markers.
//! - `review.exportFrameIo`: export composition review comments & markers to Frame.io JSON or CSV.
//! - `review.addComment`: add a frame-accurate review comment or threaded reply (`parentId`).
//! - `review.setStatus`: set `needsReview`, `inProgress`, or `approved` on a comment.
//! - `review.addAnnotation`: attach normalized `[0, 1]` Telestrator vector shapes (`freehand`,
//!   `arrow`, `rect`, `ellipse`) with RGBA stroke to a review comment.
//! - `review.list`: query all review comments of the active (or named `comp`) composition.

use effectcraft_interchange::frameio::{
    FRAMEIO_CUE_NAME, FrameIoFormat, FrameIoReviewComment, ReviewStatus, TelestratorAnnotation, TelestratorShape, export_frameio_csv, export_frameio_json,
    frame_to_timecode, import_frameio_csv, import_frameio_json, timecode_to_frame,
};
use effectcraft_project::{Comp, Marker};
use serde_json::{Value, json};

use super::{CommandSpec, b_p, bad, comp_id, has_comp, merge_p, str_p};
use crate::{EngineError, Result, Session, cmd, query};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("review.importFrameIo", "Import Frame.io Review", [], None, "{path?, data?, format?, replace?}", has_comp, import_frameio),
        query!("review.exportFrameIo", "Export Frame.io Review", "{path?, format?}", export_frameio),
        cmd!(
            "review.addComment",
            "Add Review Comment",
            [],
            None,
            "{text, author?, frame?, timecode?, durationFrames?, parentId?, status?, id?}",
            has_comp,
            add_comment
        ),
        cmd!("review.setStatus", "Set Review Comment Status", [], None, "{id, status}", has_comp, set_status),
        cmd!(
            "review.addAnnotation",
            "Add Review Telestrator Annotation",
            [],
            None,
            "{id, kind, points?, start?, end?, min?, max?, center?, radii?, color?, strokeWidth?}",
            has_comp,
            add_annotation
        ),
        query!("review.list", "List Review Comments", "{}", list_comments),
    ]
}

fn comp_start_frames(comp: &Comp) -> i64 {
    comp.frame_rate.frame_at(comp.display_start)
}

pub fn extract_comp_reviews(comp: &Comp) -> Vec<FrameIoReviewComment> {
    let rate = comp.frame_rate;
    let drop_frame = rate.supports_drop_frame();
    let start_f = comp_start_frames(comp);
    comp.markers.iter().filter_map(|m| FrameIoReviewComment::from_comp_marker(m, rate, drop_frame, start_f)).collect()
}

fn write_comp_reviews(comp: &mut Comp, comments: &[FrameIoReviewComment]) {
    let rate = comp.frame_rate;
    let mut kept: Vec<Marker> = comp.markers.iter().filter(|m| m.cue_point.as_ref().is_none_or(|q| q.name != FRAMEIO_CUE_NAME)).cloned().collect();
    for c in comments {
        kept.push(c.to_comp_marker(rate));
    }
    kept.sort_by_key(|m| m.time);
    comp.markers = kept;
}

fn next_review_id(existing: &[FrameIoReviewComment]) -> String {
    let max_n = existing.iter().filter_map(|c| c.id.strip_prefix("rev-")?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("rev-{}", max_n + 1)
}

fn import_frameio(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "review.importFrameIo";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let rate = comp.frame_rate;
    let drop_frame = rate.supports_drop_frame();
    let start_f = comp_start_frames(comp);

    let path_opt = str_p(p, "path").map(str::trim).filter(|v| !v.is_empty());
    let payload = if let Some(inline) = str_p(p, "data") {
        inline.to_string()
    } else if let Some(path) = path_opt {
        if path.contains('\0') {
            return Err(bad(CMD, "`path` must not contain NUL characters"));
        }
        let bytes = s.services.read_file(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
        String::from_utf8(bytes).map_err(|e| bad(CMD, format!("{path}: invalid UTF-8: {e}")))?
    } else {
        return Err(bad(CMD, "provide `path` or inline `data`"));
    };

    let fmt = match str_p(p, "format") {
        Some(f) => FrameIoFormat::parse(f).ok_or_else(|| bad(CMD, "format must be `json` or `csv`"))?,
        None => FrameIoFormat::detect(&payload, path_opt),
    };
    let replace = b_p(p, "replace").unwrap_or(false);

    let imported = match fmt {
        FrameIoFormat::Json => import_frameio_json(&payload, rate, drop_frame, start_f),
        FrameIoFormat::Csv => import_frameio_csv(&payload, rate, drop_frame, start_f),
    }
    .map_err(|e| bad(CMD, e.to_string()))?;

    let imported_count = imported.len();
    let mut total_count = 0_usize;
    let merge = merge_p(p).map(str::to_string);
    s.edit("Import Frame.io Review", merge.as_deref(), |pr, _| {
        let c = pr.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let mut list = if replace { Vec::new() } else { extract_comp_reviews(c) };
        for item in imported {
            if let Some(existing) = list.iter_mut().find(|r| r.id == item.id) {
                *existing = item;
            } else {
                list.push(item);
            }
        }
        total_count = list.len();
        write_comp_reviews(c, &list);
        Ok(())
    })?;

    Ok(json!({
        "format": fmt.as_str(),
        "imported": imported_count,
        "total": total_count,
    }))
}

fn export_frameio(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "review.exportFrameIo";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let comp_name = s.project.item(cid).map(|i| i.name.as_str()).unwrap_or("Composition");
    let rate = comp.frame_rate;
    let drop_frame = rate.supports_drop_frame();
    let start_f = comp_start_frames(comp);

    let path_opt = match p.get("path") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let path = v.as_str().filter(|s| !s.trim().is_empty() && !s.contains('\0')).ok_or_else(|| bad(CMD, "`path` must be a non-empty string"))?;
            Some(path)
        }
    };
    let fmt = match str_p(p, "format") {
        Some(f) => FrameIoFormat::parse(f).ok_or_else(|| bad(CMD, "format must be `json` or `csv`"))?,
        None => path_opt.and_then(|pt| pt.rsplit_once('.').and_then(|(_, ext)| FrameIoFormat::parse(ext))).unwrap_or(FrameIoFormat::Json),
    };

    let mut comments = extract_comp_reviews(comp);
    for (idx, m) in comp.markers.iter().enumerate() {
        if m.cue_point.as_ref().is_some_and(|q| q.name == FRAMEIO_CUE_NAME) {
            continue;
        }
        let mid = format!("comp-marker-{}", idx + 1);
        if comments.iter().any(|c| c.id == mid) {
            continue;
        }
        let frame = rate.frame_at(m.time).max(0);
        let duration_frames = rate.frame_at(m.duration).max(0);
        let text = if !m.comment.trim().is_empty() {
            m.comment.clone()
        } else if !m.chapter.trim().is_empty() {
            m.chapter.clone()
        } else {
            "Composition Marker".into()
        };
        comments.push(FrameIoReviewComment {
            id: mid,
            parent_id: None,
            author: "Compositor".into(),
            frame,
            timecode: frame_to_timecode(frame, rate, drop_frame, start_f),
            duration_frames,
            text,
            status: ReviewStatus::NeedsReview,
            annotations: Vec::new(),
        });
    }
    comments.sort_by(|a, b| a.frame.cmp(&b.frame).then_with(|| a.id.cmp(&b.id)));

    let data = match fmt {
        FrameIoFormat::Json => export_frameio_json(&comments, comp_name, rate, drop_frame, start_f),
        FrameIoFormat::Csv => export_frameio_csv(&comments),
    }
    .map_err(|e| bad(CMD, e.to_string()))?;

    if let Some(path) = path_opt {
        s.services.write_file(path, data.as_bytes()).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    }

    Ok(json!({
        "format": fmt.as_str(),
        "count": comments.len(),
        "bytes": data.len(),
        "path": path_opt,
        "data": data,
    }))
}

fn add_comment(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "review.addComment";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let rate = comp.frame_rate;
    let drop_frame = rate.supports_drop_frame();
    let start_f = comp_start_frames(comp);
    let existing = extract_comp_reviews(comp);
    let cti = s.state.times.get(&cid).copied().unwrap_or_default();

    let text = str_p(p, "text").map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad(CMD, "need non-empty `text`"))?.to_string();
    let author = str_p(p, "author").map(str::trim).filter(|a| !a.is_empty()).unwrap_or("Reviewer").to_string();
    let parent_id = match p.get("parentId") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let pid = v.as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(CMD, "`parentId` must be a non-empty string"))?;
            if !existing.iter().any(|c| c.id == pid) {
                return Err(bad(CMD, format!("parent comment `{pid}` does not exist in this composition")));
            }
            Some(pid.to_string())
        }
    };

    let frame = if let Some(f_val) = p.get("frame").filter(|v| !v.is_null()) {
        let f = f_val.as_i64().ok_or_else(|| bad(CMD, "`frame` must be an integer"))?;
        if f < 0 {
            return Err(bad(CMD, "`frame` must be non-negative"));
        }
        f
    } else if let Some(tc_str) = str_p(p, "timecode") {
        timecode_to_frame(tc_str, rate, start_f).ok_or_else(|| bad(CMD, format!("invalid timecode `{tc_str}`")))?
    } else if let Some(pid) = &parent_id {
        existing.iter().find(|c| &c.id == pid).map_or_else(|| rate.frame_at(cti).max(0), |parent| parent.frame)
    } else {
        rate.frame_at(cti).max(0)
    };

    let duration_frames = match p.get("durationFrames").filter(|v| !v.is_null()) {
        None => 0,
        Some(v) => v.as_i64().filter(|d| (0..=10_000_000).contains(d)).ok_or_else(|| bad(CMD, "`durationFrames` must be 0..=10000000"))?,
    };
    let status = match str_p(p, "status") {
        None => ReviewStatus::NeedsReview,
        Some(st) => ReviewStatus::parse(st).ok_or_else(|| bad(CMD, "status must be `needsReview`, `inProgress`, or `approved`"))?,
    };
    let id = match p.get("id") {
        None | Some(Value::Null) => next_review_id(&existing),
        Some(v) => {
            let custom = v.as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(CMD, "`id` must be a non-empty string"))?.to_string();
            if existing.iter().any(|c| c.id == custom) {
                return Err(bad(CMD, format!("review comment `{custom}` already exists")));
            }
            custom
        }
    };
    let timecode = frame_to_timecode(frame, rate, drop_frame, start_f);
    let comment = FrameIoReviewComment {
        id: id.clone(),
        parent_id: parent_id.clone(),
        author,
        frame,
        timecode: timecode.clone(),
        duration_frames,
        text,
        status,
        annotations: Vec::new(),
    };
    comment.validate().map_err(|e| bad(CMD, e))?;

    let merge = merge_p(p).map(str::to_string);
    s.edit("Add Review Comment", merge.as_deref(), |pr, _| {
        let c = pr.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let mut list = extract_comp_reviews(c);
        list.push(comment);
        write_comp_reviews(c, &list);
        Ok(())
    })?;

    Ok(json!({
        "id": id,
        "frame": frame,
        "timecode": timecode,
        "status": status.as_str(),
        "parentId": parent_id,
    }))
}

fn set_status(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "review.setStatus";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let id = str_p(p, "id").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(CMD, "need comment `id`"))?.to_string();
    let status_raw = str_p(p, "status").ok_or_else(|| bad(CMD, "need `status`"))?;
    let status = ReviewStatus::parse(status_raw).ok_or_else(|| bad(CMD, "status must be `needsReview`, `inProgress`, or `approved`"))?;

    let existing = extract_comp_reviews(comp);
    if !existing.iter().any(|c| c.id == id) {
        return Err(bad(CMD, format!("no review comment `{id}` in this composition")));
    }

    let merge = merge_p(p).map(str::to_string);
    s.edit("Set Review Status", merge.as_deref(), |pr, _| {
        let c = pr.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let mut list = extract_comp_reviews(c);
        if let Some(target) = list.iter_mut().find(|r| r.id == id) {
            target.status = status;
        }
        write_comp_reviews(c, &list);
        Ok(())
    })?;

    Ok(json!({ "id": id, "status": status.as_str() }))
}

fn parse_pair(v: Option<&Value>, key: &str, cmd: &str) -> Result<[f32; 2]> {
    let arr = v.and_then(Value::as_array).filter(|a| a.len() == 2).ok_or_else(|| bad(cmd, format!("`{key}` must be a [x, y] pair")))?;
    let x = arr.first().and_then(Value::as_f64).filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}[0]` must be finite")))? as f32;
    let y = arr.get(1).and_then(Value::as_f64).filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}[1]` must be finite")))? as f32;
    Ok([x, y])
}

fn parse_annotation(p: &Value, cmd: &str) -> Result<TelestratorAnnotation> {
    let kind = str_p(p, "kind").ok_or_else(|| bad(cmd, "need `kind` (`freehand`, `arrow`, `rect`, or `ellipse`)"))?;
    let shape = match kind.trim().to_ascii_lowercase().as_str() {
        "freehand" => {
            let pts_val = p.get("points").and_then(Value::as_array).ok_or_else(|| bad(cmd, "`freehand` requires `points`: [[x, y], ...]"))?;
            let mut points = Vec::with_capacity(pts_val.len());
            for pt in pts_val {
                points.push(parse_pair(Some(pt), "points[]", cmd)?);
            }
            TelestratorShape::Freehand { points }
        }
        "arrow" => TelestratorShape::Arrow { start: parse_pair(p.get("start"), "start", cmd)?, end: parse_pair(p.get("end"), "end", cmd)? },
        "rect" | "rectangle" => {
            let a = parse_pair(p.get("min"), "min", cmd)?;
            let b = parse_pair(p.get("max"), "max", cmd)?;
            TelestratorShape::Rect { min: [a[0].min(b[0]), a[1].min(b[1])], max: [a[0].max(b[0]), a[1].max(b[1])] }
        }
        "ellipse" | "circle" => {
            TelestratorShape::Ellipse { center: parse_pair(p.get("center"), "center", cmd)?, radii: parse_pair(p.get("radii"), "radii", cmd)? }
        }
        _ => return Err(bad(cmd, format!("unknown annotation kind `{kind}` (expected freehand, arrow, rect, or ellipse)"))),
    };
    let color = match p.get("color").filter(|v| !v.is_null()) {
        None => [1.0, 0.25, 0.25, 1.0],
        Some(v) => {
            let arr = v.as_array().filter(|a| a.len() == 4).ok_or_else(|| bad(cmd, "`color` must be [r, g, b, a] in [0, 1]"))?;
            let mut rgba = [0.0_f32; 4];
            for (dst, item) in rgba.iter_mut().zip(arr) {
                let f = item.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, "`color` components must be finite"))? as f32;
                *dst = f;
            }
            rgba
        }
    };
    let stroke_width = match p.get("strokeWidth").filter(|v| !v.is_null()) {
        None => 3.0,
        Some(v) => v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, "`strokeWidth` must be finite"))? as f32,
    };
    let ann = TelestratorAnnotation { shape, color, stroke_width };
    ann.validate().map_err(|e| bad(cmd, e))?;
    Ok(ann)
}

fn add_annotation(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "review.addAnnotation";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let id = str_p(p, "id").map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| bad(CMD, "need comment `id`"))?.to_string();
    let ann = parse_annotation(p, CMD)?;

    let existing = extract_comp_reviews(comp);
    if !existing.iter().any(|c| c.id == id) {
        return Err(bad(CMD, format!("no review comment `{id}` in this composition")));
    }

    let mut count = 0_usize;
    let kind_name = ann.shape.kind_name();
    let merge = merge_p(p).map(str::to_string);
    s.edit("Add Review Annotation", merge.as_deref(), |pr, _| {
        let c = pr.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let mut list = extract_comp_reviews(c);
        if let Some(target) = list.iter_mut().find(|r| r.id == id) {
            target.annotations.push(ann);
            count = target.annotations.len();
        }
        write_comp_reviews(c, &list);
        Ok(())
    })?;

    Ok(json!({ "id": id, "kind": kind_name, "annotations": count }))
}

fn list_comments(s: &mut Session, p: &Value) -> Result<Value> {
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    let comments = extract_comp_reviews(comp);
    Ok(json!({ "comments": comments }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comp_review_workflow_syncs_markers_threads_telestrator_and_undoes() {
        let mut s = Session::default();
        s.execute("comp.new", json!({ "name": "VFX_Shot_020", "fps": 24, "duration": 10 })).expect("new comp");

        let root = s
            .execute(
                "review.addComment",
                json!({
                    "author": "VFX Supervisor",
                    "frame": 48,
                    "durationFrames": 24,
                    "text": "Check muzzle flash falloff",
                }),
            )
            .expect("addComment");
        let root_id = root["id"].as_str().expect("id").to_string();
        assert_eq!(root["timecode"], json!("00:00:02:00"));

        let reply =
            s.execute("review.addComment", json!({ "author": "Compositor", "parentId": root_id, "text": "Added inverse-square glow pass" })).expect("reply");
        assert_eq!(reply["parentId"], json!(root_id));

        s.execute("review.addAnnotation", json!({ "id": root_id, "kind": "freehand", "points": [[0.1, 0.2], [0.25, 0.3], [0.4, 0.35]] })).expect("freehand");
        s.execute("review.addAnnotation", json!({ "id": root_id, "kind": "arrow", "start": [0.8, 0.2], "end": [0.55, 0.45] })).expect("arrow");
        s.execute("review.addAnnotation", json!({ "id": root_id, "kind": "rect", "min": [0.25, 0.15], "max": [0.75, 0.85] })).expect("rect");
        let ann = s.execute("review.addAnnotation", json!({ "id": root_id, "kind": "ellipse", "center": [0.5, 0.5], "radii": [0.12, 0.12] })).expect("ellipse");
        assert_eq!(ann["annotations"], json!(4));

        s.execute("review.setStatus", json!({ "id": root_id, "status": "approved" })).expect("setStatus");
        let comp = s.active_comp().expect("comp");
        assert_eq!(comp.markers.len(), 2);
        assert_eq!(comp.markers[0].label, effectcraft_color::Label::Green);

        let csv_out = s.execute("review.exportFrameIo", json!({ "format": "csv" })).expect("export csv");
        assert_eq!(csv_out["count"], json!(2));

        s.execute("comp.new", json!({ "name": "Imported_Shot", "fps": 24, "duration": 10 })).expect("new comp 2");
        let imp = s.execute("review.importFrameIo", json!({ "data": csv_out["data"], "format": "csv", "replace": true })).expect("import csv");
        assert_eq!(imp["total"], json!(2));

        assert!(s.undo());
        assert!(s.active_comp().expect("comp").markers.is_empty());
        assert!(s.redo());
        assert_eq!(s.active_comp().expect("comp").markers.len(), 2);
    }
}
