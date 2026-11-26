//! Tethered capture commands (`tether.*`): start/stop hot-folder tethering, poll for incoming
//! camera files, inspect tether status, or trigger an immediate capture poll.

use std::path::PathBuf;

use lightcraft_catalog::AlbumId;
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, bool_or, cmd, str_param};
use crate::EngineError;
use crate::tether::{TetherState, poll_watch_folder};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "tether.start",
            "Start Tethered Capture…",
            [],
            None,
            "{watchDir: str, sessionName?: str, namingPrefix?: str, startNumber?: u32, destinationDir?: str|null, albumId?: u64|null, presetId?: str|null, autoAdvance?: bool}",
            always,
            |s, p| {
                const C: &str = "tether.start";
                let raw_watch = str_param(p, "watchDir").ok_or_else(|| bad(C, "missing `watchDir`"))?;
                let watch_dir = PathBuf::from(raw_watch);
                std::fs::create_dir_all(&watch_dir).map_err(|e| EngineError::Other(format!("{}: {e}", watch_dir.display())))?;
                let session_name = str_param(p, "sessionName").unwrap_or("Untitled Shoot").trim();
                let session_name = if session_name.is_empty() { "Untitled Shoot" } else { session_name };
                let naming_prefix = str_param(p, "namingPrefix").unwrap_or(session_name).trim();
                let start_num = p.get("startNumber").and_then(Value::as_u64).unwrap_or(1).clamp(1, 999_999) as u32;
                let destination_dir = str_param(p, "destinationDir").filter(|d| !d.trim().is_empty()).map(PathBuf::from);
                if let Some(dest) = &destination_dir {
                    std::fs::create_dir_all(dest).map_err(|e| EngineError::Other(format!("{}: {e}", dest.display())))?;
                }
                let album_id = p.get("albumId").and_then(Value::as_u64).map(AlbumId);
                let preset_id = str_param(p, "presetId").filter(|id| !id.trim().is_empty()).map(str::to_string);
                let auto_advance = bool_or(p, "autoAdvance", true);

                // Mark pre-existing files in watch_dir as already seen so only shots arriving after start are imported,
                // unless `importExisting: true` is passed.
                let import_existing = bool_or(p, "importExisting", false);
                let mut ingested = std::collections::HashSet::new();
                if !import_existing
                    && let Ok(rd) = std::fs::read_dir(&watch_dir)
                {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            ingested.insert(path.to_string_lossy().to_string());
                        }
                    }
                }

                s.tether = TetherState {
                    active: true,
                    session_name: session_name.to_string(),
                    watch_dir: Some(watch_dir),
                    destination_dir,
                    naming_prefix: naming_prefix.to_string(),
                    next_sequence: start_num,
                    album_id,
                    preset_id,
                    auto_advance,
                    seen_sizes: Default::default(),
                    ingested_paths: ingested,
                    imported_ids: Vec::new(),
                    last_camera: None,
                };
                Ok(s.tether.status_json())
            }
        ),
        cmd!(
            "tether.stop",
            "Stop Tethered Capture",
            ["File", "Tethered Capture"],
            None,
            "{} — stop watching the tethered capture folder",
            always,
            |s, _| {
                s.tether.active = false;
                s.tether.seen_sizes.clear();
                Ok(s.tether.status_json())
            }
        ),
        cmd!(query "tether.status", "Tether Status", [], None, "{} — current tethered capture state and counters", always, |s, _| {
            Ok(s.tether.status_json())
        }),
        cmd!(
            "tether.poll",
            "Scan Tether Folder",
            [],
            None,
            "{requireSettled?: bool (default true)} — scan the active tether folder and import settled shots",
            always,
            |s, p| {
                let require_settled = bool_or(p, "requireSettled", true);
                poll_watch_folder(s, require_settled)
            }
        ),
        cmd!(
            "tether.capture",
            "Trigger Tether Capture",
            ["File", "Tethered Capture"],
            Some("F12"),
            "{} — trigger camera shutter / immediate tether folder ingest",
            always,
            |s, _| {
                if !s.tether.active {
                    return Err(EngineError::Disabled("tether.capture".into(), "start Tethered Capture first".into()));
                }
                // Immediate ingest of any newly written camera shot in watch_dir
                let res = poll_watch_folder(s, false)?;
                Ok(json!({
                    "captured": res["imported"],
                    "status": s.tether.status_json(),
                }))
            }
        ),
    ]
}
