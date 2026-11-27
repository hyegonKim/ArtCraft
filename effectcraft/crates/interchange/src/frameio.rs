//! Frame.io (`FFC_framemob`) team review comments, approval states, reply threading,
//! normalized `[0, 1]` Telestrator vector annotations, and JSON/CSV + Composition Marker sync.

use effectcraft_color::Label;
use effectcraft_project::{CuePoint, Marker};
use effectcraft_time::{FrameRate, format_timecode_frames, parse_timecode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_REVIEW_BYTES: usize = 16 * 1024 * 1024;
const MAX_REVIEW_COMMENTS: usize = 50_000;
const CSV_HEADER: &str = "ID,Parent ID,Author,Frame,Timecode,Duration Frames,Status,Comment,Annotations";
pub const FRAMEIO_CUE_NAME: &str = "frameio.review";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FrameIoError(pub String);

pub type FrameIoResult<T> = std::result::Result<T, FrameIoError>;

/// Review & approval status for a Frame.io comment thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStatus {
    #[default]
    #[serde(alias = "NeedsReview", alias = "needs_review")]
    NeedsReview,
    #[serde(alias = "InProgress", alias = "in_progress")]
    InProgress,
    #[serde(alias = "Approved", alias = "approved")]
    Approved,
}

impl ReviewStatus {
    pub const ALL: [ReviewStatus; 3] = [ReviewStatus::NeedsReview, ReviewStatus::InProgress, ReviewStatus::Approved];

    pub fn as_str(self) -> &'static str {
        match self {
            ReviewStatus::NeedsReview => "needsReview",
            ReviewStatus::InProgress => "inProgress",
            ReviewStatus::Approved => "approved",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ReviewStatus::NeedsReview => "Needs Review",
            ReviewStatus::InProgress => "In Progress",
            ReviewStatus::Approved => "Approved",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
            "needsreview" | "open" | "pending" | "todo" => Some(ReviewStatus::NeedsReview),
            "inprogress" | "working" | "wip" => Some(ReviewStatus::InProgress),
            "approved" | "done" | "resolved" | "completed" => Some(ReviewStatus::Approved),
            _ => None,
        }
    }

    /// Composition marker label color corresponding to this review status.
    pub fn marker_label(self) -> Label {
        match self {
            ReviewStatus::NeedsReview => Label::Orange,
            ReviewStatus::InProgress => Label::Cyan,
            ReviewStatus::Approved => Label::Green,
        }
    }
}

/// Normalized `[0, 1]` on-frame vector Telestrator geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TelestratorShape {
    #[serde(alias = "Freehand")]
    Freehand { points: Vec<[f32; 2]> },
    #[serde(alias = "Arrow")]
    Arrow { start: [f32; 2], end: [f32; 2] },
    #[serde(alias = "Rect")]
    Rect { min: [f32; 2], max: [f32; 2] },
    #[serde(alias = "Ellipse")]
    Ellipse { center: [f32; 2], radii: [f32; 2] },
}

impl TelestratorShape {
    pub fn kind_name(&self) -> &'static str {
        match self {
            TelestratorShape::Freehand { .. } => "freehand",
            TelestratorShape::Arrow { .. } => "arrow",
            TelestratorShape::Rect { .. } => "rect",
            TelestratorShape::Ellipse { .. } => "ellipse",
        }
    }
}

/// On-frame vector Telestrator stroke with normalized `[0, 1]` coordinates and RGBA color.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelestratorAnnotation {
    pub shape: TelestratorShape,
    /// Straight sRGB RGBA stroke color in `[0.0, 1.0]`.
    pub color: [f32; 4],
    /// Stroke thickness in logical points (`0.25..=64.0`).
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
}

fn default_stroke_width() -> f32 {
    3.0
}

fn valid_unit_pt(p: [f32; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite() && (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1])
}

impl TelestratorAnnotation {
    pub const MAX_FREEHAND_POINTS: usize = 4096;

    pub fn validate(&self) -> Result<(), String> {
        for c in self.color {
            if !c.is_finite() || !(0.0..=1.0).contains(&c) {
                return Err("annotation RGBA color components must be finite numbers in [0, 1]".into());
            }
        }
        if !self.stroke_width.is_finite() || !(0.25..=64.0).contains(&self.stroke_width) {
            return Err("annotation strokeWidth must be between 0.25 and 64.0".into());
        }
        match &self.shape {
            TelestratorShape::Freehand { points } => {
                if points.len() < 2 || points.len() > Self::MAX_FREEHAND_POINTS {
                    return Err(format!("freehand annotation needs 2..={} normalized points", Self::MAX_FREEHAND_POINTS));
                }
                if !points.iter().copied().all(valid_unit_pt) {
                    return Err("freehand points must have normalized coordinates in [0, 1]".into());
                }
            }
            TelestratorShape::Arrow { start, end } => {
                if !valid_unit_pt(*start) || !valid_unit_pt(*end) {
                    return Err("arrow start and end must have normalized coordinates in [0, 1]".into());
                }
            }
            TelestratorShape::Rect { min, max } => {
                if !valid_unit_pt(*min) || !valid_unit_pt(*max) {
                    return Err("rect min and max must have normalized coordinates in [0, 1]".into());
                }
                if min[0] > max[0] || min[1] > max[1] {
                    return Err("rect min must not exceed max".into());
                }
            }
            TelestratorShape::Ellipse { center, radii } => {
                if !valid_unit_pt(*center) {
                    return Err("ellipse center must have normalized coordinates in [0, 1]".into());
                }
                if !radii[0].is_finite() || !radii[1].is_finite() || !(0.0..=1.0).contains(&radii[0]) || !(0.0..=1.0).contains(&radii[1]) {
                    return Err("ellipse radii must be finite numbers in [0, 1]".into());
                }
            }
        }
        Ok(())
    }
}

/// A Frame.io review comment or threaded reply anchored to a composition frame / timecode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameIoReviewComment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub author: String,
    pub frame: i64,
    pub timecode: String,
    #[serde(default)]
    pub duration_frames: i64,
    pub text: String,
    #[serde(default)]
    pub status: ReviewStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<TelestratorAnnotation>,
}

impl FrameIoReviewComment {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.id.len() > 256 || self.id.chars().any(char::is_control) {
            return Err("review comment id must be 1..=256 printable characters".into());
        }
        if let Some(pid) = &self.parent_id
            && (pid.trim().is_empty() || pid == &self.id || pid.len() > 256 || pid.chars().any(char::is_control))
        {
            return Err("parentId must name a distinct printable comment id".into());
        }
        if self.author.len() > 512 || self.author.chars().any(char::is_control) {
            return Err("author must be at most 512 printable bytes".into());
        }
        if self.frame < 0 || self.duration_frames < 0 || self.duration_frames > 10_000_000 {
            return Err("frame and durationFrames must be non-negative within supported bounds".into());
        }
        if self.text.trim().is_empty() || self.text.len() > 65_536 {
            return Err("comment text must be 1..=65536 bytes".into());
        }
        if self.annotations.len() > 256 {
            return Err("a single comment may carry at most 256 annotations".into());
        }
        for ann in &self.annotations {
            ann.validate()?;
        }
        Ok(())
    }

    /// Encode this review comment as a lossless composition `Marker` with `CuePoint` metadata.
    pub fn to_comp_marker(&self, rate: FrameRate) -> Marker {
        let ann_json = if self.annotations.is_empty() { String::new() } else { serde_json::to_string(&self.annotations).unwrap_or_default() };
        let mut params = vec![
            ("id".into(), self.id.clone()),
            ("author".into(), self.author.clone()),
            ("status".into(), self.status.as_str().into()),
            ("text".into(), self.text.clone()),
            ("timecode".into(), self.timecode.clone()),
        ];
        if let Some(pid) = &self.parent_id {
            params.push(("parentId".into(), pid.clone()));
        }
        if !ann_json.is_empty() {
            params.push(("annotations".into(), ann_json));
        }
        let mut display = format!("[{}] {}: {}", self.status.label(), self.author, self.text);
        if !self.annotations.is_empty() {
            display.push_str(&format!(" ({} drawing(s))", self.annotations.len()));
        }
        Marker {
            time: rate.tick_of(self.frame.max(0)),
            duration: rate.tick_of(self.duration_frames.max(0)),
            comment: display,
            chapter: String::new(),
            url: String::new(),
            frame_target: String::new(),
            cue_point: Some(CuePoint { name: FRAMEIO_CUE_NAME.into(), navigation: false, params }),
            protected: false,
            label: self.status.marker_label(),
        }
    }

    /// Extract a `FrameIoReviewComment` from a composition `Marker` carrying a `frameio.review` cue point.
    pub fn from_comp_marker(m: &Marker, rate: FrameRate, drop_frame: bool, start_tc_frames: i64) -> Option<Self> {
        let cue = m.cue_point.as_ref().filter(|q| q.name == FRAMEIO_CUE_NAME)?;
        let get = |key: &str| cue.params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let id = get("id")?.to_string();
        let parent_id = get("parentId").filter(|s| !s.is_empty()).map(str::to_string);
        let author = get("author").unwrap_or("Reviewer").to_string();
        let status = get("status").and_then(ReviewStatus::parse).unwrap_or(ReviewStatus::NeedsReview);
        let text = get("text").unwrap_or(&m.comment).to_string();
        let frame = rate.frame_at(m.time).max(0);
        let duration_frames = rate.frame_at(m.duration).max(0);
        let timecode =
            get("timecode").filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(|| frame_to_timecode(frame, rate, drop_frame, start_tc_frames));
        let annotations =
            get("annotations").filter(|s| !s.is_empty()).and_then(|s| serde_json::from_str::<Vec<TelestratorAnnotation>>(s).ok()).unwrap_or_default();
        Some(FrameIoReviewComment { id, parent_id, author, frame, timecode, duration_frames, text, status, annotations })
    }
}

/// Top-level review comments sorted by `(frame, id)`.
pub fn root_comments(comments: &[FrameIoReviewComment]) -> Vec<&FrameIoReviewComment> {
    let ids: std::collections::BTreeSet<&str> = comments.iter().map(|c| c.id.as_str()).collect();
    let mut roots: Vec<&FrameIoReviewComment> = comments.iter().filter(|c| c.parent_id.as_deref().is_none_or(|pid| !ids.contains(pid))).collect();
    roots.sort_by(|a, b| a.frame.cmp(&b.frame).then_with(|| a.id.cmp(&b.id)));
    roots
}

/// Direct replies to `parent_id` in chronological/id order.
pub fn replies_to<'a>(comments: &'a [FrameIoReviewComment], parent_id: &str) -> Vec<&'a FrameIoReviewComment> {
    let mut out: Vec<&FrameIoReviewComment> = comments.iter().filter(|c| c.parent_id.as_deref() == Some(parent_id)).collect();
    out.sort_by(|a, b| a.frame.cmp(&b.frame).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Review interchange wire format (`json` or `csv`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameIoFormat {
    Json,
    Csv,
}

impl FrameIoFormat {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "json" | "frameio" => Some(FrameIoFormat::Json),
            "csv" => Some(FrameIoFormat::Csv),
            _ => None,
        }
    }

    pub fn detect(data: &str, path_hint: Option<&str>) -> Self {
        if let Some(p) = path_hint
            && let Some((_, ext)) = p.rsplit_once('.')
            && let Some(fmt) = Self::parse(ext)
        {
            return fmt;
        }
        let trimmed = data.trim_start_matches('\u{feff}').trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') { FrameIoFormat::Json } else { FrameIoFormat::Csv }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FrameIoFormat::Json => "json",
            FrameIoFormat::Csv => "csv",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameIoRateMeta {
    num: i64,
    den: i64,
    drop_frame: bool,
    #[serde(default)]
    start_timecode: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameIoEnvelope {
    schema: String,
    asset: String,
    frame_rate: FrameIoRateMeta,
    comments: Vec<FrameIoReviewComment>,
}

pub fn frame_to_timecode(frame: i64, rate: FrameRate, drop_frame: bool, start_tc_frames: i64) -> String {
    let total = frame.max(0).saturating_add(start_tc_frames.max(0));
    let raw = format_timecode_frames(total, rate, drop_frame && rate.supports_drop_frame());
    if drop_frame && rate.supports_drop_frame() { raw } else { raw.replace(';', ":") }
}

pub fn timecode_to_frame(tc: &str, rate: FrameRate, start_tc_frames: i64) -> Option<i64> {
    let trimmed = tc.trim();
    let abs_frame = parse_timecode(trimmed, rate, trimmed.contains(';'), 0).ok()?;
    Some((abs_frame - start_tc_frames).max(0))
}

pub fn export_frameio_json(
    comments: &[FrameIoReviewComment],
    asset_name: &str,
    rate: FrameRate,
    drop_frame: bool,
    start_tc_frames: i64,
) -> FrameIoResult<String> {
    if comments.len() > MAX_REVIEW_COMMENTS {
        return Err(FrameIoError(format!("too many review comments (maximum {MAX_REVIEW_COMMENTS})")));
    }
    let mut normalized = Vec::with_capacity(comments.len());
    for c in comments {
        c.validate().map_err(FrameIoError)?;
        let mut item = c.clone();
        if item.timecode.trim().is_empty() {
            item.timecode = frame_to_timecode(item.frame, rate, drop_frame, start_tc_frames);
        }
        normalized.push(item);
    }
    let env = FrameIoEnvelope {
        schema: "frameio.review.v1".into(),
        asset: asset_name.into(),
        frame_rate: FrameIoRateMeta { num: rate.num, den: rate.den, drop_frame, start_timecode: start_tc_frames },
        comments: normalized,
    };
    let out = serde_json::to_string_pretty(&env).map_err(|e| FrameIoError(format!("JSON serialization failed: {e}")))?;
    if out.len() > MAX_REVIEW_BYTES {
        return Err(FrameIoError("Frame.io JSON export exceeds 16 MiB limit".into()));
    }
    Ok(out)
}

fn parse_external_comment(v: &Value, index: usize, rate: FrameRate, drop_frame: bool, start_tc_frames: i64) -> FrameIoResult<FrameIoReviewComment> {
    if let Ok(mut direct) = serde_json::from_value::<FrameIoReviewComment>(v.clone()) {
        if direct.timecode.trim().is_empty() {
            direct.timecode = frame_to_timecode(direct.frame, rate, drop_frame, start_tc_frames);
        } else if v.get("frame").is_none()
            && let Some(parsed_f) = timecode_to_frame(&direct.timecode, rate, start_tc_frames)
        {
            direct.frame = parsed_f;
        }
        if direct.author.trim().is_empty() {
            direct.author = "Reviewer".into();
        }
        direct.validate().map_err(FrameIoError)?;
        return Ok(direct);
    }

    let obj = v.as_object().ok_or_else(|| FrameIoError(format!("comment #{index} must be a JSON object")))?;
    let id = obj.get("id").or_else(|| obj.get("comment_id")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("fio-{}", index + 1));
    let parent_id = obj.get("parentId").or_else(|| obj.get("parent_id")).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let author = obj
        .get("author")
        .and_then(Value::as_str)
        .or_else(|| obj.get("owner").and_then(|o| o.get("name")).and_then(Value::as_str))
        .unwrap_or("Reviewer")
        .to_string();
    let text = obj.get("text").or_else(|| obj.get("comment")).and_then(Value::as_str).unwrap_or_default().to_string();
    let raw_tc = obj.get("timecode").and_then(Value::as_str).unwrap_or_default();
    let frame = obj
        .get("frame")
        .or_else(|| obj.get("timestamp"))
        .and_then(Value::as_i64)
        .or_else(|| (!raw_tc.is_empty()).then(|| timecode_to_frame(raw_tc, rate, start_tc_frames)).flatten())
        .unwrap_or(0);
    let timecode = if raw_tc.trim().is_empty() { frame_to_timecode(frame, rate, drop_frame, start_tc_frames) } else { raw_tc.to_string() };
    let duration_frames = obj.get("durationFrames").or_else(|| obj.get("duration_frames")).or_else(|| obj.get("duration")).and_then(Value::as_i64).unwrap_or(0);
    let status = if let Some(st_str) = obj.get("status").and_then(Value::as_str) {
        ReviewStatus::parse(st_str).ok_or_else(|| FrameIoError(format!("unknown review status `{st_str}`")))?
    } else if obj.get("completed").and_then(Value::as_bool) == Some(true) {
        ReviewStatus::Approved
    } else {
        ReviewStatus::NeedsReview
    };
    let annotations = match obj.get("annotations") {
        None | Some(Value::Null) => Vec::new(),
        Some(arr) => {
            serde_json::from_value::<Vec<TelestratorAnnotation>>(arr.clone()).map_err(|e| FrameIoError(format!("invalid annotations on `{id}`: {e}")))?
        }
    };
    let comment = FrameIoReviewComment { id, parent_id, author, frame, timecode, duration_frames, text, status, annotations };
    comment.validate().map_err(FrameIoError)?;
    Ok(comment)
}

pub fn import_frameio_json(data: &str, rate: FrameRate, drop_frame: bool, start_tc_frames: i64) -> FrameIoResult<Vec<FrameIoReviewComment>> {
    if data.len() > MAX_REVIEW_BYTES {
        return Err(FrameIoError("Frame.io JSON exceeds 16 MiB limit".into()));
    }
    let trimmed = data.trim_start_matches('\u{feff}').trim();
    let root: Value = serde_json::from_str(trimmed).map_err(|e| FrameIoError(format!("Frame.io JSON parse error: {e}")))?;
    let (items, eff_rate, eff_df, eff_start) = match &root {
        Value::Array(arr) => (arr.as_slice(), rate, drop_frame, start_tc_frames),
        Value::Object(map) => {
            let arr = map
                .get("comments")
                .or_else(|| map.get("data"))
                .and_then(Value::as_array)
                .ok_or_else(|| FrameIoError("Frame.io JSON object must contain a `comments` array".into()))?;
            let (r, df, st) = if let Some(fr) = map.get("frameRate").and_then(Value::as_object) {
                let num = fr.get("num").and_then(Value::as_i64).unwrap_or(rate.num);
                let den = fr.get("den").and_then(Value::as_i64).unwrap_or(rate.den);
                let valid = if num > 0 && den > 0 { FrameRate::new(num, den) } else { rate };
                let d = fr.get("dropFrame").and_then(Value::as_bool).unwrap_or(drop_frame);
                let s = fr.get("startTimecode").and_then(Value::as_i64).unwrap_or(start_tc_frames);
                (valid, d, s)
            } else {
                (rate, drop_frame, start_tc_frames)
            };
            (arr.as_slice(), r, df, st)
        }
        _ => return Err(FrameIoError("Frame.io JSON root must be an object or array".into())),
    };
    if items.len() > MAX_REVIEW_COMMENTS {
        return Err(FrameIoError(format!("too many review comments (maximum {MAX_REVIEW_COMMENTS})")));
    }
    let mut out = Vec::with_capacity(items.len());
    let mut seen = std::collections::BTreeSet::new();
    for (idx, item) in items.iter().enumerate() {
        let parsed = parse_external_comment(item, idx, eff_rate, eff_df, eff_start)?;
        if !seen.insert(parsed.id.clone()) {
            return Err(FrameIoError(format!("duplicate review comment id `{}`", parsed.id)));
        }
        out.push(parsed);
    }
    Ok(out)
}

fn push_csv_cell(out: &mut String, field: &str, escape_formula: bool) {
    out.push('"');
    if escape_formula && field.trim_start().starts_with(['=', '+', '-', '@']) {
        out.push('\'');
    }
    for ch in field.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
}

fn unescape_formula_cell(cell: &str) -> String {
    if let Some(rest) = cell.strip_prefix('\'')
        && rest.trim_start().starts_with(['=', '+', '-', '@'])
    {
        return rest.to_string();
    }
    cell.to_string()
}

pub fn export_frameio_csv(comments: &[FrameIoReviewComment]) -> FrameIoResult<String> {
    if comments.len() > MAX_REVIEW_COMMENTS {
        return Err(FrameIoError(format!("too many review comments (maximum {MAX_REVIEW_COMMENTS})")));
    }
    let mut csv = String::with_capacity(256 + comments.len() * 128);
    csv.push_str(CSV_HEADER);
    csv.push_str("\r\n");
    for c in comments {
        c.validate().map_err(FrameIoError)?;
        let ann_json = if c.annotations.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&c.annotations).map_err(|e| FrameIoError(format!("annotation serialize error: {e}")))?
        };
        let cells: [(&str, bool); 9] = [
            (&c.id, true),
            (c.parent_id.as_deref().unwrap_or(""), true),
            (&c.author, true),
            (&c.frame.to_string(), false),
            (&c.timecode, false),
            (&c.duration_frames.to_string(), false),
            (c.status.as_str(), false),
            (&c.text, true),
            (&ann_json, false),
        ];
        for (i, (val, esc_formula)) in cells.iter().enumerate() {
            if i > 0 {
                csv.push(',');
            }
            push_csv_cell(&mut csv, val, *esc_formula);
        }
        csv.push_str("\r\n");
        if csv.len() > MAX_REVIEW_BYTES {
            return Err(FrameIoError("Frame.io CSV export exceeds 16 MiB limit".into()));
        }
    }
    Ok(csv)
}

fn parse_rfc4180_rows(input: &str) -> FrameIoResult<Vec<Vec<String>>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_quotes = false;
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next();
                    cell.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                cell.push(ch);
            }
        } else {
            match ch {
                '"' if cell.is_empty() => in_quotes = true,
                ',' => row.push(std::mem::take(&mut cell)),
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        let _ = chars.next();
                    }
                    row.push(std::mem::take(&mut cell));
                    if row.len() > 1 || row.first().is_some_and(|s| !s.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                '\n' => {
                    row.push(std::mem::take(&mut cell));
                    if row.len() > 1 || row.first().is_some_and(|s| !s.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                other => cell.push(other),
            }
        }
    }
    if in_quotes {
        return Err(FrameIoError("unterminated quoted field in Frame.io CSV".into()));
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        if row.len() > 1 || row.first().is_some_and(|s| !s.trim().is_empty()) {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn find_col(headers: &[String], names: &[&str]) -> Option<usize> {
    headers.iter().position(|h| {
        let norm = h.trim().to_ascii_lowercase().replace([' ', '_', '-'], "");
        names.iter().any(|cand| norm == *cand)
    })
}

pub fn import_frameio_csv(csv_text: &str, rate: FrameRate, drop_frame: bool, start_tc_frames: i64) -> FrameIoResult<Vec<FrameIoReviewComment>> {
    if csv_text.len() > MAX_REVIEW_BYTES {
        return Err(FrameIoError("Frame.io CSV exceeds 16 MiB limit".into()));
    }
    let rows = parse_rfc4180_rows(csv_text)?;
    let Some((header, data_rows)) = rows.split_first() else {
        return Ok(Vec::new());
    };
    if data_rows.len() > MAX_REVIEW_COMMENTS {
        return Err(FrameIoError(format!("too many CSV rows (maximum {MAX_REVIEW_COMMENTS})")));
    }

    let c_id = find_col(header, &["id", "commentid"]);
    let c_parent = find_col(header, &["parentid", "replyto", "parent"]);
    let c_author = find_col(header, &["author", "user", "reviewer", "commenter", "owner"]);
    let c_frame = find_col(header, &["frame", "startframe", "framein"]);
    let c_tc = find_col(header, &["timecode", "tc", "timecodein", "starttimecode"]);
    let c_dur = find_col(header, &["durationframes", "duration"]);
    let c_status = find_col(header, &["status", "state", "completed"]);
    let c_text =
        find_col(header, &["comment", "text", "note", "body"]).ok_or_else(|| FrameIoError("Frame.io CSV requires a `Comment` or `Text` column".into()))?;
    let c_ann = find_col(header, &["annotations", "telestrator", "drawings"]);

    let cell = |r: &[String], idx: Option<usize>| -> String { idx.and_then(|i| r.get(i)).cloned().unwrap_or_default() };

    let mut out = Vec::with_capacity(data_rows.len());
    let mut seen = std::collections::BTreeSet::new();
    for (row_idx, r) in data_rows.iter().enumerate() {
        let raw_id = unescape_formula_cell(cell(r, c_id).trim());
        let id = if raw_id.is_empty() { format!("fio-{}", row_idx + 1) } else { raw_id };
        let parent_raw = unescape_formula_cell(cell(r, c_parent).trim());
        let parent_id = (!parent_raw.is_empty()).then_some(parent_raw);
        let author_raw = unescape_formula_cell(cell(r, c_author).trim());
        let author = if author_raw.is_empty() { "Reviewer".into() } else { author_raw };
        let tc_raw = cell(r, c_tc).trim().to_string();
        let frame_str = cell(r, c_frame);
        let frame = if !frame_str.trim().is_empty() {
            frame_str.trim().parse::<i64>().map_err(|_| FrameIoError(format!("invalid frame `{}` on row {}", frame_str.trim(), row_idx + 2)))?
        } else if !tc_raw.is_empty() {
            timecode_to_frame(&tc_raw, rate, start_tc_frames).ok_or_else(|| FrameIoError(format!("invalid timecode `{tc_raw}` on row {}", row_idx + 2)))?
        } else {
            0
        };
        let timecode = if tc_raw.is_empty() { frame_to_timecode(frame, rate, drop_frame, start_tc_frames) } else { tc_raw };
        let dur_str = cell(r, c_dur);
        let duration_frames = if dur_str.trim().is_empty() {
            0
        } else {
            dur_str.trim().parse::<i64>().map_err(|_| FrameIoError(format!("invalid durationFrames `{}` on row {}", dur_str.trim(), row_idx + 2)))?
        };
        let st_str = cell(r, c_status);
        let status = if st_str.trim().is_empty() {
            ReviewStatus::NeedsReview
        } else {
            ReviewStatus::parse(&st_str).ok_or_else(|| FrameIoError(format!("unknown review status `{}` on row {}", st_str.trim(), row_idx + 2)))?
        };
        let text = unescape_formula_cell(&cell(r, Some(c_text)));
        let ann_str = cell(r, c_ann);
        let annotations = if ann_str.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<TelestratorAnnotation>>(ann_str.trim())
                .map_err(|e| FrameIoError(format!("invalid annotations JSON on row {}: {e}", row_idx + 2)))?
        };
        let comment = FrameIoReviewComment { id, parent_id, author, frame, timecode, duration_frames, text, status, annotations };
        comment.validate().map_err(FrameIoError)?;
        if !seen.insert(comment.id.clone()) {
            return Err(FrameIoError(format!("duplicate review comment id `{}`", comment.id)));
        }
        out.push(comment);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_csv_and_comp_marker_round_trip_preserve_telestrator_and_threads() {
        let rate = FrameRate::new(24, 1);
        let comments = vec![
            FrameIoReviewComment {
                id: "ec-rev-1".into(),
                parent_id: None,
                author: "VFX Sup".into(),
                frame: 48,
                timecode: "00:00:02:00".into(),
                duration_frames: 12,
                text: "Clean matte edge, \"spill\" on hair\r\n+despill".into(),
                status: ReviewStatus::InProgress,
                annotations: vec![
                    TelestratorAnnotation {
                        shape: TelestratorShape::Freehand { points: vec![[0.2, 0.2], [0.3, 0.25], [0.4, 0.35]] },
                        color: [1.0, 0.2, 0.2, 1.0],
                        stroke_width: 3.0,
                    },
                    TelestratorAnnotation {
                        shape: TelestratorShape::Arrow { start: [0.8, 0.15], end: [0.55, 0.3] },
                        color: [1.0, 0.85, 0.1, 1.0],
                        stroke_width: 3.5,
                    },
                    TelestratorAnnotation {
                        shape: TelestratorShape::Rect { min: [0.4, 0.1], max: [0.7, 0.5] },
                        color: [0.2, 0.9, 0.4, 1.0],
                        stroke_width: 2.0,
                    },
                    TelestratorAnnotation {
                        shape: TelestratorShape::Ellipse { center: [0.5, 0.5], radii: [0.1, 0.15] },
                        color: [0.2, 0.7, 1.0, 1.0],
                        stroke_width: 2.5,
                    },
                ],
            },
            FrameIoReviewComment {
                id: "ec-rev-2".into(),
                parent_id: Some("ec-rev-1".into()),
                author: "Comp Artist".into(),
                frame: 48,
                timecode: "00:00:02:00".into(),
                duration_frames: 0,
                text: "Despilled & refined alpha.".into(),
                status: ReviewStatus::Approved,
                annotations: Vec::new(),
            },
        ];

        let json_str = export_frameio_json(&comments, "Shot_010_v04", rate, false, 0).expect("export json");
        assert_eq!(import_frameio_json(&json_str, rate, false, 0).expect("import json"), comments);

        let csv_str = export_frameio_csv(&comments).expect("export csv");
        assert_eq!(import_frameio_csv(&csv_str, rate, false, 0).expect("import csv"), comments);

        let marker = comments[0].to_comp_marker(rate);
        let recovered = FrameIoReviewComment::from_comp_marker(&marker, rate, false, 0).expect("from marker");
        assert_eq!(recovered, comments[0]);
    }
}
