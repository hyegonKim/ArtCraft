//! Tethered capture & camera hot-folder session state.
//!
//! Monitors a watched incoming camera folder (USB/Wi-Fi camera transfer utilities or tethered SDK
//! hot-folders), settles incoming files across scans (or immediately when requested), optionally
//! copies and renames shots sequentially into a session destination folder, applies a develop
//! preset and target album on arrival, and advances the active selection to the newest capture.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lightcraft_catalog::{AlbumId, PhotoId};
use serde_json::{Value, json};

use crate::import::{ImportMode, import, is_supported};
use crate::{EngineError, Result, Selection, Session};

#[derive(Clone, Debug)]
pub struct TetherState {
    pub active: bool,
    pub session_name: String,
    pub watch_dir: Option<PathBuf>,
    pub destination_dir: Option<PathBuf>,
    pub naming_prefix: String,
    pub next_sequence: u32,
    pub album_id: Option<AlbumId>,
    pub preset_id: Option<String>,
    pub auto_advance: bool,
    /// Incoming files seen during the previous scan (`path -> byte_len`).
    pub seen_sizes: HashMap<String, u64>,
    /// Incoming source paths already ingested in this tether session.
    pub ingested_paths: HashSet<String>,
    pub imported_ids: Vec<PhotoId>,
    pub last_camera: Option<String>,
}

impl Default for TetherState {
    fn default() -> Self {
        Self {
            active: false,
            session_name: "Untitled Shoot".to_string(),
            watch_dir: None,
            destination_dir: None,
            naming_prefix: "Shoot".to_string(),
            next_sequence: 1,
            album_id: None,
            preset_id: None,
            auto_advance: true,
            seen_sizes: HashMap::new(),
            ingested_paths: HashSet::new(),
            imported_ids: Vec::new(),
            last_camera: None,
        }
    }
}

impl TetherState {
    pub fn status_json(&self) -> Value {
        json!({
            "active": self.active,
            "sessionName": self.session_name,
            "watchDir": self.watch_dir.as_ref().map(|p| p.to_string_lossy()),
            "destinationDir": self.destination_dir.as_ref().map(|p| p.to_string_lossy()),
            "namingPrefix": self.naming_prefix,
            "nextSequence": self.next_sequence,
            "albumId": self.album_id.map(|a| a.0),
            "presetId": self.preset_id,
            "autoAdvance": self.auto_advance,
            "importedCount": self.imported_ids.len(),
            "lastPhotoId": self.imported_ids.last().map(|p| p.0),
            "lastCamera": self.last_camera,
        })
    }
}

fn sanitize_prefix(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() { "Shoot".to_string() } else { trimmed.to_string() }
}

fn copy_sequential(src: &Path, dest_dir: &Path, prefix: &str, seq: &mut u32) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir).map_err(|e| EngineError::Other(format!("{}: {e}", dest_dir.display())))?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("dng");
    let safe_prefix = sanitize_prefix(prefix);
    for _ in 0..10_000 {
        let candidate = dest_dir.join(format!("{safe_prefix}_{seq:04}.{ext}"));
        *seq = seq.saturating_add(1);
        if !candidate.exists() {
            std::fs::copy(src, &candidate).map_err(|e| EngineError::Other(format!("copy {} -> {}: {e}", src.display(), candidate.display())))?;
            return Ok(candidate);
        }
    }
    Err(EngineError::Other("could not allocate a free sequential filename in destination folder".into()))
}

/// Poll the active tether watch folder, importing newly settled shots.
pub fn poll_watch_folder(s: &mut Session, require_settled: bool) -> Result<Value> {
    if !s.tether.active {
        return Ok(json!({"active": false, "imported": [], "pending": 0, "totalImported": s.tether.imported_ids.len()}));
    }
    let Some(watch_dir) = s.tether.watch_dir.clone() else {
        return Err(EngineError::Other("tether session has no watched folder".into()));
    };
    let entries = std::fs::read_dir(&watch_dir).map_err(|e| EngineError::Other(format!("{}: {e}", watch_dir.display())))?;
    let mut files: Vec<(PathBuf, String, u64)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_supported(&path) {
            continue;
        }
        let key = path.to_string_lossy().to_string();
        if s.tether.ingested_paths.contains(&key) {
            continue;
        }
        // Skip photos already catalogued at this exact path
        let already_in_catalog = s.catalog.photos().any(|p| matches!(&p.source, lightcraft_catalog::Source::File { path: existing } if existing == &key));
        if already_in_catalog {
            s.tether.ingested_paths.insert(key);
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() == 0 {
            continue;
        }
        files.push((path, key, meta.len()));
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));

    let mut ready: Vec<(PathBuf, String)> = Vec::new();
    let mut pending = 0usize;
    let mut next_seen = HashMap::new();
    for (path, key, len) in files {
        if !require_settled || s.tether.seen_sizes.get(&key).copied() == Some(len) {
            ready.push((path, key));
        } else {
            next_seen.insert(key, len);
            pending += 1;
        }
    }
    s.tether.seen_sizes = next_seen;

    let mut newly_imported: Vec<PhotoId> = Vec::new();
    let dest_dir = s.tether.destination_dir.clone();
    let prefix = s.tether.naming_prefix.clone();
    let preset_id = s.tether.preset_id.clone();
    let album_id = s.tether.album_id;
    let auto_advance = s.tether.auto_advance;

    for (src_path, key) in ready {
        let target_path = if let Some(dest) = &dest_dir {
            if dest != &watch_dir {
                copy_sequential(&src_path, dest, &prefix, &mut s.tether.next_sequence)?
            } else {
                s.tether.next_sequence = s.tether.next_sequence.saturating_add(1);
                src_path
            }
        } else {
            s.tether.next_sequence = s.tether.next_sequence.saturating_add(1);
            src_path
        };
        let path_str = target_path.to_string_lossy().to_string();
        let report = import(s, std::slice::from_ref(&path_str), ImportMode::Add)?;
        s.tether.ingested_paths.insert(key);
        for raw_id in report.imported {
            let pid = PhotoId(raw_id);
            if let Some(cam) = s.catalog.photo(pid).map(|p| p.meta.camera.trim().to_string()).filter(|c| !c.is_empty()) {
                s.tether.last_camera = Some(cam);
            }
            if let Some(pr_id) = &preset_id
                && let Some(pr) = s.presets.iter().find(|p| &p.id == pr_id).cloned()
                && let Some(cur) = s.develop_of(pid)
            {
                let next = pr.apply(&cur, 1.0);
                let _ = s.set_develop(pid, next, &pr.name);
            }
            if let Some(aid) = album_id
                && let Some(album) = s.catalog.album(aid)
                && !album.folder
                && !album.is_smart()
                && !album.photos.contains(&pid)
            {
                let mut photos = album.photos.clone();
                photos.push(pid);
                let _ = s.commit("Add Tethered Shot to Album", lightcraft_catalog::Op::SetAlbumPhotos { id: aid, photos });
            }
            s.tether.imported_ids.push(pid);
            newly_imported.push(pid);
        }
    }

    if auto_advance
        && let Some(&last) = newly_imported.last()
    {
        s.selection = Selection::single(last);
    }

    Ok(json!({
        "active": s.tether.active,
        "imported": newly_imported.iter().map(|p| p.0).collect::<Vec<_>>(),
        "pending": pending,
        "totalImported": s.tether.imported_ids.len(),
        "lastPhotoId": s.tether.imported_ids.last().map(|p| p.0),
        "lastCamera": s.tether.last_camera,
    }))
}
