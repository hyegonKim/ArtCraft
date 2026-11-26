//! Photoshop 2027 (28.0) Timelapse recording & export (`Window › Timelapse`, `File › Export › Export Timelapse…`).
//!
//! Captures scaled RGBA8 composite snapshots into a bounded per-document ring buffer as edits are
//! committed (`Session::edit_with`), and encodes the recorded sequence as MP4, WebM, AVI or GIF.
//! Resolution and Quality are locked once recording starts until `timelapse.clear` resets the
//! session, matching Photoshop 2027's Timelapse panel behaviour.

use std::collections::VecDeque;

use photocraft_doc::Document;
use photocraft_io::{
    TimelapseFormat, TimelapseQuality, TimelapseRawFrame, TimelapseResolution, encode_timelapse_frames, timelapse_target_dimensions,
};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::prefs::{FileHandling, TimelapseAutoStart, TimelapseQualPref, TimelapseResPref};
use crate::{EngineError, Result, Session};

/// Default cap on stored timelapse frames per open document.
pub const DEFAULT_MAX_TIMELAPSE_FRAMES: usize = 360;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimelapseState {
    #[default]
    Idle,
    Recording,
    Paused,
}

impl TimelapseState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Paused => "paused",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelapseFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct TimelapseRecorder {
    pub state: TimelapseState,
    pub resolution: TimelapseResolution,
    pub quality: TimelapseQuality,
    /// Once recording starts, resolution and quality cannot be changed until cleared.
    pub locked_after_start: bool,
    pub max_frames: usize,
    pub frames: VecDeque<TimelapseFrame>,
}

impl Default for TimelapseRecorder {
    fn default() -> Self {
        Self {
            state: TimelapseState::Idle,
            resolution: TimelapseResolution::Res1920,
            quality: TimelapseQuality::High,
            locked_after_start: false,
            max_frames: DEFAULT_MAX_TIMELAPSE_FRAMES,
            frames: VecDeque::new(),
        }
    }
}

impl TimelapseRecorder {
    pub fn is_capturing(&self) -> bool {
        self.state == TimelapseState::Recording
    }

    pub fn resolution_str(&self) -> &'static str {
        match self.resolution {
            TimelapseResolution::Res2560 => "2560",
            TimelapseResolution::Res1920 => "1920",
        }
    }

    pub fn quality_str(&self) -> &'static str {
        match self.quality {
            TimelapseQuality::High => "high",
            TimelapseQuality::Medium => "medium",
            TimelapseQuality::Low => "low",
        }
    }

    /// Initialize defaults and optional auto-start from `Preferences › File Handling`.
    pub fn init_from_prefs(&mut self, fh: &FileHandling, is_new_doc: bool, initial_doc: &Document) {
        self.resolution = match fh.timelapse_resolution {
            TimelapseResPref::Res2560 => TimelapseResolution::Res2560,
            TimelapseResPref::Res1920 => TimelapseResolution::Res1920,
        };
        self.quality = match fh.timelapse_quality {
            TimelapseQualPref::High => TimelapseQuality::High,
            TimelapseQualPref::Medium => TimelapseQuality::Medium,
            TimelapseQualPref::Low => TimelapseQuality::Low,
        };
        let auto = match fh.timelapse_auto_start {
            TimelapseAutoStart::StartAll => true,
            TimelapseAutoStart::StartNew => is_new_doc,
            TimelapseAutoStart::StartManual => false,
        };
        if auto {
            self.start(initial_doc);
        }
    }

    pub fn start(&mut self, doc: &Document) {
        self.state = TimelapseState::Recording;
        self.locked_after_start = true;
        if self.frames.is_empty() {
            self.capture_document(doc);
        }
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.state = TimelapseState::Idle;
        self.locked_after_start = false;
    }

    /// Render and append a composite frame of `doc` if it differs from the previous frame.
    pub fn capture_document(&mut self, doc: &Document) {
        if doc.size.width == 0 || doc.size.height == 0 {
            return;
        }
        let (tw, th) = timelapse_target_dimensions(doc.size.width, doc.size.height, self.resolution);
        let thumb = photocraft_compose::thumbnail(doc, self.resolution.long_edge());
        let rgba = resize_rgba_nearest(&thumb.pixels, thumb.width, thumb.height, tw, th);
        if self.frames.back().is_some_and(|last| last.width == tw && last.height == th && last.rgba == rgba) {
            return;
        }
        let cap = self.max_frames.max(2);
        if self.frames.len() >= cap {
            // Preserve frame 0 (initial document state) and evict the oldest intermediate frame.
            let _ = self.frames.remove(1);
        }
        self.frames.push_back(TimelapseFrame { width: tw, height: th, rgba });
    }
}

fn resize_rgba_nearest(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if sw == dw && sh == dh && src.len() == (dw as usize).saturating_mul(dh as usize).saturating_mul(4) {
        return src.to_vec();
    }
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || src.len() < (sw as usize).saturating_mul(sh as usize).saturating_mul(4) {
        return vec![0u8; (dw as usize).saturating_mul(dh as usize).saturating_mul(4)];
    }
    let mut out = vec![0u8; (dw as usize).saturating_mul(dh as usize).saturating_mul(4)];
    for y in 0..dh {
        let sy = ((u64::from(y) * u64::from(sh)) / u64::from(dh)).min(u64::from(sh - 1)) as usize;
        for x in 0..dw {
            let sx = ((u64::from(x) * u64::from(sw)) / u64::from(dw)).min(u64::from(sw - 1)) as usize;
            let si = (sy * sw as usize + sx) * 4;
            let di = (y as usize * dw as usize + x as usize) * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn can_export(s: &Session) -> std::result::Result<(), String> {
    crate::file_cmds::native(s)?;
    let st = s.active().ok_or_else(|| "no document open".to_string())?;
    if st.timelapse.frames.is_empty() {
        Err("no timelapse frames recorded".into())
    } else {
        Ok(())
    }
}

fn status_json(rec: &TimelapseRecorder) -> Value {
    json!({
        "state": rec.state.as_str(),
        "frameCount": rec.frames.len(),
        "resolution": rec.resolution_str(),
        "quality": rec.quality_str(),
        "lockedAfterStart": rec.locked_after_start,
        "maxFrames": rec.max_frames,
    })
}

/// Encode the active document's recorded timelapse frames to container bytes.
pub fn export_timelapse_bytes(
    rec: &TimelapseRecorder,
    fps: u32,
    format: TimelapseFormat,
) -> Result<(Vec<u8>, u32, u32, usize)> {
    let last = rec.frames.back().ok_or_else(|| EngineError::Other("no timelapse frames recorded".into()))?;
    let (tw, th) = timelapse_target_dimensions(last.width.max(2), last.height.max(2), rec.resolution);
    let raw_frames: Vec<TimelapseRawFrame> = rec
        .frames
        .iter()
        .map(|f| TimelapseRawFrame {
            width: tw,
            height: th,
            rgba8: resize_rgba_nearest(&f.rgba, f.width, f.height, tw, th),
        })
        .collect();
    let count = raw_frames.len();
    let bytes = encode_timelapse_frames(&raw_frames, rec.resolution, rec.quality, fps.clamp(1, 60), format)
        .map_err(|e| EngineError::Other(format!("timelapse export failed: {e}")))?;
    Ok((bytes, tw, th, count))
}

fn parse_format(p: &Value, path: &str, cmd: &str) -> Result<TimelapseFormat> {
    if let Some(fmt_str) = p.get("format").and_then(Value::as_str) {
        return match fmt_str.to_ascii_lowercase().as_str() {
            "mp4" => Ok(TimelapseFormat::Mp4),
            "webm" => Ok(TimelapseFormat::Webm),
            "avi" => Ok(TimelapseFormat::Avi),
            "gif" => Ok(TimelapseFormat::Gif),
            other => Err(bad(cmd, format!("unknown timelapse format `{other}` (expected mp4|webm|avi|gif)"))),
        };
    }
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    Ok(match ext.as_str() {
        "webm" => TimelapseFormat::Webm,
        "avi" => TimelapseFormat::Avi,
        "gif" => TimelapseFormat::Gif,
        _ => TimelapseFormat::Mp4,
    })
}

fn format_name(fmt: TimelapseFormat) -> &'static str {
    match fmt {
        TimelapseFormat::Mp4 => "mp4",
        TimelapseFormat::Webm => "webm",
        TimelapseFormat::Avi => "avi",
        TimelapseFormat::Gif => "gif",
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "timelapse.start",
            label: "Start Timelapse",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: has_doc,
            run: |s, _| {
                let st = s.active_mut().ok_or(EngineError::NoDocument)?;
                let doc = st.doc.clone();
                st.timelapse.start(&doc);
                st.view_revision += 1;
                st.revision += 1;
                st.saved_revision = if st.saved_revision + 1 == st.revision { st.revision } else { st.saved_revision };
                Ok(status_json(&st.timelapse))
            },
            journal: true,
        },
        CommandSpec {
            id: "timelapse.pause",
            label: "Pause Timelapse",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: |s| {
                let st = s.active().ok_or("no document open")?;
                if st.timelapse.state == TimelapseState::Recording {
                    Ok(())
                } else {
                    Err("timelapse is not currently recording".into())
                }
            },
            run: |s, _| {
                let st = s.active_mut().ok_or(EngineError::NoDocument)?;
                if st.timelapse.state != TimelapseState::Recording {
                    return Err(EngineError::Other("timelapse is not currently recording".into()));
                }
                st.timelapse.state = TimelapseState::Paused;
                st.view_revision += 1;
                Ok(status_json(&st.timelapse))
            },
            journal: true,
        },
        CommandSpec {
            id: "timelapse.resume",
            label: "Resume Timelapse",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: |s| {
                let st = s.active().ok_or("no document open")?;
                if st.timelapse.state == TimelapseState::Paused {
                    Ok(())
                } else {
                    Err("timelapse is not paused".into())
                }
            },
            run: |s, _| {
                let st = s.active_mut().ok_or(EngineError::NoDocument)?;
                if st.timelapse.state != TimelapseState::Paused {
                    return Err(EngineError::Other("timelapse is not paused".into()));
                }
                st.timelapse.state = TimelapseState::Recording;
                st.view_revision += 1;
                Ok(status_json(&st.timelapse))
            },
            journal: true,
        },
        CommandSpec {
            id: "timelapse.clear",
            label: "Clear Timelapse",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: has_doc,
            run: |s, _| {
                let st = s.active_mut().ok_or(EngineError::NoDocument)?;
                st.timelapse.clear();
                st.view_revision += 1;
                Ok(status_json(&st.timelapse))
            },
            journal: true,
        },
        CommandSpec {
            id: "timelapse.configure",
            label: "Configure Timelapse",
            menu: &[],
            shortcut: None,
            params: r#"{"resolution":"2560|1920"?,"quality":"high|medium|low"?,"maxFrames":u32?}"#,
            enabled: has_doc,
            run: |s, p| {
                const CMD: &str = "timelapse.configure";
                let st = s.active_mut().ok_or(EngineError::NoDocument)?;
                let wants_res_or_qual = p.get("resolution").is_some() || p.get("quality").is_some();
                if wants_res_or_qual && st.timelapse.locked_after_start {
                    return Err(EngineError::Other(
                        "Timelapse resolution and quality cannot be changed after recording has started; clear the recording first".into(),
                    ));
                }
                if let Some(res_val) = p.get("resolution") {
                    let res_str = res_val.as_str().ok_or_else(|| bad(CMD, "`resolution` must be \"2560\" or \"1920\""))?;
                    st.timelapse.resolution = match res_str {
                        "2560" | "2k" | "2K" => TimelapseResolution::Res2560,
                        "1920" | "1080p" => TimelapseResolution::Res1920,
                        other => return Err(bad(CMD, format!("unknown resolution `{other}` (expected \"2560\" or \"1920\")"))),
                    };
                }
                if let Some(q_val) = p.get("quality") {
                    let q_str = q_val.as_str().ok_or_else(|| bad(CMD, "`quality` must be \"high\", \"medium\" or \"low\""))?;
                    st.timelapse.quality = match q_str {
                        "high" => TimelapseQuality::High,
                        "medium" => TimelapseQuality::Medium,
                        "low" => TimelapseQuality::Low,
                        other => return Err(bad(CMD, format!("unknown quality `{other}` (expected \"high\", \"medium\" or \"low\")"))),
                    };
                }
                if let Some(mf) = p.get("maxFrames").and_then(Value::as_u64) {
                    st.timelapse.max_frames = (mf as usize).clamp(2, 3600);
                }
                st.view_revision += 1;
                Ok(status_json(&st.timelapse))
            },
            journal: true,
        },
        CommandSpec {
            id: "timelapse.status",
            label: "Timelapse Status",
            menu: &[],
            shortcut: None,
            params: "{}",
            enabled: has_doc,
            run: |s, _| {
                let st = s.active().ok_or(EngineError::NoDocument)?;
                Ok(status_json(&st.timelapse))
            },
            journal: false,
        },
        CommandSpec {
            id: "file.export.timelapse",
            label: "Export Timelapse…",
            menu: &["File", "Export"],
            shortcut: None,
            params: r#"{"path":str,"fps":1..60=30,"format":"mp4|webm|avi|gif"?}"#,
            enabled: can_export,
            run: |s, p| {
                const CMD: &str = "file.export.timelapse";
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| bad(CMD, "missing `path`"))?
                    .to_string();
                let fps = p.get("fps").and_then(Value::as_u64).map_or(30, |v| v.clamp(1, 60) as u32);
                let fmt = parse_format(p, &path, CMD)?;
                let st = s.active().ok_or(EngineError::NoDocument)?;
                let (bytes, tw, th, frames) = export_timelapse_bytes(&st.timelapse, fps, fmt)?;
                crate::file_cmds::write_file(&path, &bytes)?;
                Ok(json!({
                    "path": path,
                    "frames": frames,
                    "width": tw,
                    "height": th,
                    "fps": fps,
                    "format": format_name(fmt),
                    "bytes": bytes.len(),
                }))
            },
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn timelapse_records_edits_locks_settings_pauses_and_exports_mp4() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 64, "height": 48, "background": "white"})).unwrap();
        s.execute("timelapse.configure", json!({"resolution": "2560", "quality": "medium"})).unwrap();
        let st = s.execute("timelapse.start", json!({})).unwrap();
        assert_eq!(st["state"], "recording");
        assert_eq!(st["frameCount"], 1);
        assert_eq!(st["lockedAfterStart"], true);

        // Changing resolution/quality while locked must fail, matching Photoshop 2027.
        assert!(s.execute("timelapse.configure", json!({"resolution": "1920"})).is_err());

        // Perform a visible edit -> captured automatically!
        s.execute("edit.fill", json!({"contents": "color", "color": "#ff2200"})).unwrap();
        assert_eq!(s.execute("timelapse.status", json!({})).unwrap()["frameCount"], 2);

        // Pause -> edits do not add frames while paused.
        s.execute("timelapse.pause", json!({})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#0055ff"})).unwrap();
        assert_eq!(s.execute("timelapse.status", json!({})).unwrap()["frameCount"], 2);

        // Resume -> subsequent edits record again.
        s.execute("timelapse.resume", json!({})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#22cc66"})).unwrap();
        assert_eq!(s.execute("timelapse.status", json!({})).unwrap()["frameCount"], 3);

        // Export to temporary MP4 and GIF files.
        let dir = std::env::temp_dir();
        let mp4_path = dir.join(format!("pcraft_tl_test_{}.mp4", std::process::id()));
        let mp4_str = mp4_path.to_string_lossy().to_string();
        let out = s.execute("file.export.timelapse", json!({"path": mp4_str, "fps": 24})).unwrap();
        assert_eq!(out["frames"], 3);
        assert_eq!(out["format"], "mp4");
        let written = std::fs::read(&mp4_path).unwrap();
        let _ = std::fs::remove_file(&mp4_path);
        assert!(written.len() > 64 && &written[4..8] == b"ftyp");

        // Clear unlocks configuration again.
        s.execute("timelapse.clear", json!({})).unwrap();
        let after = s.execute("timelapse.configure", json!({"resolution": "1920", "quality": "high"})).unwrap();
        assert_eq!(after["frameCount"], 0);
        assert_eq!(after["lockedAfterStart"], false);
    }

    #[test]
    fn timelapse_auto_start_preferences_honoured_on_new_documents() {
        let mut s = Session::new();
        s.execute("prefs.set", json!({"path": "fileHandling.timelapseAutoStart", "value": "startNew"})).unwrap();
        s.execute("prefs.set", json!({"path": "fileHandling.timelapseResolution", "value": "2560"})).unwrap();
        s.execute("prefs.set", json!({"path": "fileHandling.timelapseQuality", "value": "low"})).unwrap();
        s.execute("file.new", json!({"width": 32, "height": 32})).unwrap();
        let status = s.execute("timelapse.status", json!({})).unwrap();
        assert_eq!(status["state"], "recording");
        assert_eq!(status["resolution"], "2560");
        assert_eq!(status["quality"], "low");
        assert_eq!(status["frameCount"], 1);
    }
}
