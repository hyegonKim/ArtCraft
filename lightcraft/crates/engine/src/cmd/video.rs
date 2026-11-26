//! Video clip trim & poster-frame commands (`video.trim`, `video.trimInfo`).

use lightcraft_catalog::{MediaKind, Op, PhotoId};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_active, photo_id};
use crate::{EngineError, Result, Session};

fn resolve_target(s: &Session, p: &Value, c: &str) -> Result<PhotoId> {
    match photo_id(p, c)? {
        Some(id) => Ok(id),
        None => s.active().ok_or_else(|| bad(c, "no active photo")),
    }
}

fn trim_json(p: &lightcraft_catalog::Photo) -> Value {
    let dur = p.duration;
    let in_sec = p.meta.video_in;
    let out_sec = p.meta.video_out;
    let poster_sec = p.meta.video_poster;
    let eff_in = in_sec.unwrap_or(0.0);
    let eff_out = out_sec.or(dur);
    let trimmed_dur = eff_out.map(|o| (o - eff_in).max(0.0));
    json!({
        "id": p.id.0,
        "isVideo": p.kind == MediaKind::Video,
        "durationSec": dur,
        "inSec": in_sec,
        "outSec": out_sec,
        "posterSec": poster_sec,
        "trimmedDurationSec": trimmed_dur,
        "trimmedDuration": trimmed_dur,
    })
}

fn opt_sec(p: &Value, key: &str, c: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => {
            let sec = v.as_f64().ok_or_else(|| bad(c, format!("`{key}` must be a non-negative number or null")))?;
            if !sec.is_finite() || sec < 0.0 {
                return Err(bad(c, format!("`{key}` must be finite and >= 0")));
            }
            Ok(Some(Some(sec)))
        }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                query "video.trimInfo",
                "Video Trim Info",
                [],
                None,
                "{id?} — clip duration, in/out trim points, and poster frame timestamp",
                has_active,
                |s, p| {
                    const C: &str = "video.trimInfo";
                    let id = resolve_target(s, p, C)?;
                    let photo = s.catalog.photo(id).ok_or(lightcraft_catalog::CatalogError::NoPhoto(id))?;
                    Ok(trim_json(photo))
                }
            )
        },
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "video.trim",
                "Trim Video",
                [],
                None,
                "{id?, inSec?: f64|null, outSec?: f64|null, posterSec?: f64|null, reset?: bool} — set video in/out points and poster frame",
                has_active,
                |s, p| {
                    const C: &str = "video.trim";
                    let id = resolve_target(s, p, C)?;
                    let photo = s.catalog.photo(id).ok_or(lightcraft_catalog::CatalogError::NoPhoto(id))?;
                    if photo.kind != MediaKind::Video {
                        return Err(EngineError::Other(format!("{} is not a video clip", photo.file_name)));
                    }
                    let dur = photo.duration;
                    let mut meta = photo.meta.clone();
                    if p.get("reset").and_then(Value::as_bool) == Some(true) {
                        meta.video_in = None;
                        meta.video_out = None;
                        meta.video_poster = None;
                    } else {
                        if let Some(v) = opt_sec(p, "inSec", C)? {
                            meta.video_in = v.map(|t| match dur {
                                Some(d) if d > 0.0 => t.min(d),
                                _ => t,
                            });
                        }
                        if let Some(v) = opt_sec(p, "outSec", C)? {
                            meta.video_out = v.map(|t| match dur {
                                Some(d) if d > 0.0 => t.min(d),
                                _ => t,
                            });
                        }
                        if let Some(v) = opt_sec(p, "posterSec", C)? {
                            meta.video_poster = v.map(|t| match dur {
                                Some(d) if d > 0.0 => t.min(d),
                                _ => t,
                            });
                        }
                    }
                    if let (Some(i), Some(o)) = (meta.video_in, meta.video_out)
                        && o <= i + 1e-3
                    {
                        return Err(bad(C, "`outSec` must be greater than `inSec`"));
                    }
                    if meta != photo.meta {
                        s.skip_auto_write = true;
                        s.commit("Trim Video", Op::SetMeta { id, meta: Box::new(meta) })?;
                    }
                    let updated = s.catalog.photo(id).ok_or(lightcraft_catalog::CatalogError::NoPhoto(id))?;
                    Ok(trim_json(updated))
                }
            )
        },
    ]
}
