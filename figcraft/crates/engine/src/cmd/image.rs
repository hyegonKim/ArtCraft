//! Image and video paints: adjustment presets, built in and saved in the file, Crop image, and Video fill upload/place.

use figcraft_color::{ImageFilters, ImageScaleMode, Paint, PaintKind, VideoPaint};
use figcraft_doc::{Document, ImagePreset, Node, NodeKind};
use serde_json::{Value, json};

use super::{CommandSpec, ok};
use crate::params::{boolean, f64_of, get, num, num_or, opt_id, req_index, req_string, req_targets, string, targets};
use crate::{EngineError, Result, Session, bad, spec};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "image.adjust",
            "Adjust image",
            "{ids?, target? (fill|stroke, default fill), index? (default every image paint), preset? (vivid|muted|warm|cool|blackAndWhite|fade|reset, or the name of a preset saved in the file), filters? ({exposure, contrast, saturation, temperature, tint, highlights, shadows}: each -1..1, applied over the preset)}  sets the adjustments of the layers' image paints; returns {changed}",
            true,
            adjust
        ),
        spec!(
            "image.replace",
            "Replace image",
            "{ids?, target? (fill|stroke, default fill), index? (default each layer's top image paint), data (base64) | path}  puts the image in the paint, keeping its fill mode, crop, position and adjustments; a paint at `index` that isn't an image becomes one, filling the layer; returns {image, width, height, changed}",
            true,
            replace
        ),
        spec!(
            "video.upload",
            "Upload video",
            "{ids?, target? (fill|stroke, default fill), index?, data? (base64) | path?, posterData? (base64), mode? (fill|fit|crop|tile), autoplay?, loopPlayback?, muted?}  sets or replaces a video fill on the target layers; returns {video, poster, width, height, durationSecs, changed}",
            true,
            upload_video
        ),
        spec!(
            "video.place",
            "Place video",
            "{data? (base64) | path?, posterData? (base64), x?, y?, width?, height?, parent?, name?, target?, mode? (fill|fit|crop|tile), autoplay?, loopPlayback?, muted?}  places a video-filled rectangle or adds a video fill to `target`; returns {id, video, poster, width, height, durationSecs}",
            true,
            place_video
        ),
        spec!(
            "image.savePreset",
            "Save image preset",
            "{name, filters? (default the adjustments of the first image paint of `ids`/the selection), ids?, target?, index?}  saved in the file; a preset with the same name is replaced",
            true,
            save_preset
        ),
        spec!("image.deletePreset", "Delete image preset", "{name}", true, |s, p| {
            let name = req_string("image.deletePreset", p, "name")?;
            let doc = s.doc_mut()?;
            let before = doc.image_presets.len();
            doc.image_presets.retain(|x| x.name != name);
            if doc.image_presets.len() == before {
                return Err(bad("image.deletePreset", format!("no preset named `{name}`")));
            }
            ok()
        }),
        spec!(
            "image.crop",
            "Crop image",
            "{ids? (one layer), index? (an image fill; default its top visible one)} → {id, index}: the image fill switches to Crop, keeping the picture where it was, and the canvas enters crop mode on it",
            true,
            crop
        ),
        spec!("image.presets", "Image presets", "{} → {builtIn: [{id, label, filters}], saved: [{name, filters}]}", false, |s, _| {
            let built_in: Vec<Value> = ImageFilters::PRESETS.iter().map(|(id, label, f)| json!({"id": id, "label": label, "filters": f})).collect();
            Ok(json!({"builtIn": built_in, "saved": s.doc()?.image_presets}))
        }),
    ]
}

/// The longest name a saved preset may have.
const MAX_NAME: usize = 100;
/// The most presets a file keeps.
const MAX_PRESETS: usize = 500;

/// Whether `p` asks for strokes (`target: stroke`) rather than fills.
fn strokes(cmd: &str, p: &Value) -> Result<bool> {
    match string(p, "target").as_deref() {
        None | Some("fill") | Some("fills") => Ok(false),
        Some("stroke") | Some("strokes") => Ok(true),
        Some(_) => Err(bad(cmd, "target: fill|stroke")),
    }
}

/// The preset `name`: one saved in the file under that exact name, else a built-in one.
pub fn find_preset(doc: &Document, name: &str) -> Option<ImageFilters> {
    doc.image_presets.iter().find(|x| x.name == name).map(|x| x.filters).or_else(|| ImageFilters::preset(name))
}

/// `base` with the adjustments `v` names (an object of -1..1 numbers) set.
fn with_values(cmd: &str, base: ImageFilters, v: &Value) -> Result<ImageFilters> {
    let Value::Object(m) = v else { return Err(bad(cmd, "filters: an object of adjustments")) };
    let mut f = base;
    for (k, val) in m {
        let x = f64_of(val).ok_or_else(|| bad(cmd, format!("filters.{k}: a number")))? as f32;
        let slot = match k.as_str() {
            "exposure" => &mut f.exposure,
            "contrast" => &mut f.contrast,
            "saturation" => &mut f.saturation,
            "temperature" => &mut f.temperature,
            "tint" => &mut f.tint,
            "highlights" => &mut f.highlights,
            "shadows" => &mut f.shadows,
            _ => return Err(bad(cmd, format!("unknown adjustment `{k}`"))),
        };
        *slot = x;
    }
    Ok(f.sanitized())
}

fn adjust(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "image.adjust";
    let ids = req_targets(cmd, s, p)?;
    let stroke = strokes(cmd, p)?;
    let index = match get(p, "index") {
        None | Some(Value::Null) => None,
        Some(_) => Some(req_index(cmd, p, "index")?),
    };
    let preset = match string(p, "preset") {
        Some(name) => Some(find_preset(s.doc()?, &name).ok_or_else(|| bad(cmd, format!("no preset named `{name}`")))?),
        None => None,
    };
    let values = get(p, "filters").filter(|v| !v.is_null());
    if preset.is_none() && values.is_none() {
        return Err(bad(cmd, "pass `preset` or `filters`"));
    }
    let doc = s.doc_mut()?;
    let mut changed = 0;
    for id in ids {
        let n = doc.get(id)?;
        if !if stroke { &n.strokes } else { &n.fills }.iter().any(|x| matches!(x.kind, PaintKind::Image(_))) {
            continue;
        }
        let n = doc.get_mut(id)?;
        let list = if stroke { &mut n.strokes } else { &mut n.fills };
        let mut touched = false;
        for (i, paint) in list.iter_mut().enumerate() {
            let PaintKind::Image(img) = &mut paint.kind else { continue };
            if index.is_some_and(|k| k != i) {
                continue;
            }
            let base = preset.unwrap_or(img.filters);
            img.filters = match values {
                Some(v) => with_values(cmd, base, v)?,
                None => base,
            };
            touched = true;
            changed += 1;
        }
        if touched {
            // Like any paint edit, the layer leaves its paint style.
            if stroke {
                n.styles.stroke = None;
            } else {
                n.styles.fill = None;
            }
        }
    }
    if changed == 0 {
        return Err(bad(cmd, if index.is_some() { "no image paint at that index" } else { "the layers have no image paints" }));
    }
    Ok(json!({"changed": changed}))
}

fn replace(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "image.replace";
    let ids = req_targets(cmd, s, p)?;
    let stroke = strokes(cmd, p)?;
    let index = match get(p, "index") {
        None | Some(Value::Null) => None,
        Some(_) => Some(req_index(cmd, p, "index")?),
    };
    let bytes = super::create::image_bytes(cmd, p)?;
    let doc = s.doc_mut()?;
    let (image, width, height) = crate::cmd::create::add_readable_image(doc, bytes).map_err(|e| bad(cmd, e))?;
    let mut changed = 0;
    for id in ids {
        let n = doc.get_mut(id)?;
        let list = if stroke { &mut n.strokes } else { &mut n.fills };
        let at = index.or_else(|| list.iter().rposition(|x| matches!(x.kind, PaintKind::Image(_))));
        let Some(paint) = at.and_then(|i| list.get_mut(i)) else { continue };
        match &mut paint.kind {
            PaintKind::Image(img) => img.image.clone_from(&image),
            kind => *kind = Paint::image(image.clone(), ImageScaleMode::Fill).kind,
        }
        // Like any paint edit, the layer leaves its paint style.
        if stroke {
            n.styles.stroke = None;
        } else {
            n.styles.fill = None;
        }
        changed += 1;
    }
    if changed == 0 {
        return Err(bad(cmd, if index.is_some() { "no paint at that index" } else { "the layers have no image paints" }));
    }
    Ok(json!({"image": image, "width": width, "height": height, "changed": changed}))
}

/// The index of a layer's top visible image fill among the fills it shows (fills paint bottom to
/// top): a text's characters' shared list, so none while they are coloured differently.
pub fn top_image_fill(n: &Node) -> Option<usize> {
    crate::cmd::lists::character_fills(n)?.iter().rposition(|f| f.visible && matches!(f.kind, PaintKind::Image(_)))
}

fn crop(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "image.crop";
    let ids = req_targets(cmd, s, p)?;
    // A selection with nothing to crop is refused as unavailable, not as bad input.
    let refuse = |why: &str| EngineError::Disabled(cmd.into(), why.into());
    let [id] = ids[..] else { return Err(refuse("select one layer to crop its image")) };
    let doc = s.doc()?;
    let n = doc.get(id)?;
    // A text's fills are its characters' (the Design panel's rows): the crop edits that list, and
    // characters coloured differently have no row to crop ("Click + to replace mixed content").
    let shown =
        crate::cmd::lists::character_fills(n).ok_or_else(|| refuse("the characters have different fills, so there is no one fill to crop"))?;
    let index = match get(p, "index") {
        None | Some(Value::Null) => top_image_fill(n).ok_or_else(|| refuse("the layer has no image fill to crop"))?,
        Some(_) => req_index(cmd, p, "index")?,
    };
    let mut paint = shown.get(index).cloned().ok_or_else(|| bad(cmd, "no fill at that index"))?;
    let PaintKind::Image(img) = &mut paint.kind else { return Err(bad(cmd, "that fill is not an image")) };
    // An image already in Crop mode only enters crop mode: no edit, no undo step.
    if img.scale_mode != ImageScaleMode::Crop {
        let data = doc.images.get(&img.image).ok_or_else(|| bad(cmd, "the image's data is missing"))?;
        img.crop = Some(img.placement_crop(f64::from(data.width), f64::from(data.height), n.width, n.height));
        img.scale_mode = ImageScaleMode::Crop;
        let n = s.doc_mut()?.get_mut(id)?;
        // The characters' shared list becomes the layer's, so the crop shows on every one of them.
        crate::cmd::lists::take_character_fills(n);
        if let Some(f) = n.fills.get_mut(index) {
            *f = paint;
        }
        // Like any paint edit, the layer leaves its paint style.
        n.styles.fill = None;
    }
    s.request_ui(cmd, json!({"id": id, "index": index}));
    Ok(json!({"id": id, "index": index}))
}

fn save_preset(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "image.savePreset";
    let name: String = req_string(cmd, p, "name")?.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
    if name.is_empty() {
        return Err(bad(cmd, "`name` is empty"));
    }
    let filters = match get(p, "filters").filter(|v| !v.is_null()) {
        Some(v) => with_values(cmd, ImageFilters::NONE, v)?,
        None => {
            let stroke = strokes(cmd, p)?;
            let index = match get(p, "index") {
                None | Some(Value::Null) => None,
                Some(_) => Some(req_index(cmd, p, "index")?),
            };
            let doc = s.doc()?;
            targets(s, p)
                .iter()
                .filter_map(|id| doc.node(*id))
                .flat_map(|n| if stroke { n.strokes.iter() } else { n.fills.iter() }.enumerate())
                .find_map(|(i, x)| match &x.kind {
                    PaintKind::Image(img) if index.is_none_or(|k| k == i) => Some(img.filters),
                    _ => None,
                })
                .ok_or_else(|| bad(cmd, "pass `filters`, or select a layer with an image paint"))?
        }
    };
    let doc = s.doc_mut()?;
    let preset = ImagePreset { name: name.clone(), filters };
    match doc.image_presets.iter().position(|x| x.name == name) {
        Some(i) => {
            if let Some(x) = doc.image_presets.get_mut(i) {
                *x = preset;
            }
        }
        None if doc.image_presets.len() >= MAX_PRESETS => return Err(bad(cmd, format!("a file keeps at most {MAX_PRESETS} presets"))),
        None => doc.image_presets.push(preset),
    }
    Ok(json!({"name": name, "filters": filters}))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_video_path(path: &str) -> Result<Vec<u8>> {
    crate::file_access::read_capped(path, figcraft_doc::VideoData::MAX_BYTES)
}

#[cfg(target_arch = "wasm32")]
fn read_video_path(path: &str) -> Result<Vec<u8>> {
    Err(EngineError::Other(format!("{path}: files can't be read in the browser")))
}

fn decode_base64_field(cmd: &str, p: &Value, key: &str) -> Result<Option<Vec<u8>>> {
    let Some(b64) = string(p, key) else { return Ok(None) };
    use base64::Engine as _;
    let b64 = b64.split_once(',').map(|(_, d)| d.to_string()).unwrap_or(b64);
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map(Some)
        .map_err(|e| bad(cmd, format!("`{key}`: {e}")))
}

fn video_bytes(cmd: &str, p: &Value) -> Result<(Vec<u8>, Option<String>)> {
    if let Some(bytes) = decode_base64_field(cmd, p, "data")? {
        Ok((bytes, None))
    } else if let Some(path) = string(p, "path") {
        let stem = std::path::Path::new(&path)
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_string);
        Ok((read_video_path(&path)?, stem))
    } else {
        Err(bad(cmd, "pass `data` (base64) or `path`"))
    }
}

fn scale_mode_param(p: &Value) -> ImageScaleMode {
    match string(p, "mode").unwrap_or_default().to_ascii_lowercase().as_str() {
        "fit" => ImageScaleMode::Fit,
        "crop" => ImageScaleMode::Crop,
        "tile" => ImageScaleMode::Tile,
        _ => ImageScaleMode::Fill,
    }
}

fn register_video_with_poster(
    cmd: &str,
    doc: &mut Document,
    bytes: Vec<u8>,
    poster_bytes: Option<Vec<u8>>,
) -> Result<(String, String, u32, u32, f64)> {
    if let Some(ref pb) = poster_bytes {
        let _ = crate::cmd::create::add_readable_image(doc, pb.clone()).map_err(|e| bad(cmd, e))?;
    }
    figcraft_format::add_video(doc, bytes, poster_bytes).map_err(|e| bad(cmd, e.to_string()))
}

fn upload_video(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "video.upload";
    let ids = req_targets(cmd, s, p)?;
    let stroke = strokes(cmd, p)?;
    let index = match get(p, "index") {
        None | Some(Value::Null) => None,
        Some(_) => Some(req_index(cmd, p, "index")?),
    };
    let (bytes, _stem) = video_bytes(cmd, p)?;
    let poster_bytes = decode_base64_field(cmd, p, "posterData")?;
    let mode_override = string(p, "mode").map(|_| scale_mode_param(p));
    let autoplay = boolean(p, "autoplay");
    let loop_playback = boolean(p, "loopPlayback");
    let muted = boolean(p, "muted");

    let doc = s.doc_mut()?;
    let (video, poster, width, height, duration_secs) =
        register_video_with_poster(cmd, doc, bytes, poster_bytes)?;
    let mut changed = 0;
    for id in ids {
        let n = doc.get_mut(id)?;
        let list = if stroke { &mut n.strokes } else { &mut n.fills };
        let at = index
            .or_else(|| list.iter().rposition(|x| matches!(x.kind, PaintKind::Video(_))))
            .or_else(|| (!list.is_empty()).then(|| list.len() - 1));
        if let Some(paint) = at.and_then(|i| list.get_mut(i)) {
            match &mut paint.kind {
                PaintKind::Video(vid) => {
                    vid.video.clone_from(&video);
                    vid.poster = Some(poster.clone());
                    if let Some(m) = mode_override {
                        vid.scale_mode = m;
                    }
                    if let Some(a) = autoplay {
                        vid.autoplay = a;
                    }
                    if let Some(l) = loop_playback {
                        vid.loop_playback = l;
                    }
                    if let Some(m) = muted {
                        vid.muted = m;
                    }
                }
                kind => {
                    let mut vid = VideoPaint::new(
                        video.clone(),
                        Some(poster.clone()),
                        mode_override.unwrap_or(ImageScaleMode::Fill),
                    );
                    if let Some(a) = autoplay {
                        vid.autoplay = a;
                    }
                    if let Some(l) = loop_playback {
                        vid.loop_playback = l;
                    }
                    if let Some(m) = muted {
                        vid.muted = m;
                    }
                    *kind = PaintKind::Video(vid);
                }
            }
        } else if index.is_none() {
            let mut vid = VideoPaint::new(
                video.clone(),
                Some(poster.clone()),
                mode_override.unwrap_or(ImageScaleMode::Fill),
            );
            if let Some(a) = autoplay {
                vid.autoplay = a;
            }
            if let Some(l) = loop_playback {
                vid.loop_playback = l;
            }
            if let Some(m) = muted {
                vid.muted = m;
            }
            list.push(Paint { kind: PaintKind::Video(vid), ..Paint::solid(figcraft_color::Color::WHITE) });
        } else {
            continue;
        }
        if stroke {
            n.styles.stroke = None;
        } else {
            n.styles.fill = None;
        }
        changed += 1;
    }
    if changed == 0 {
        return Err(bad(cmd, "no paint at that index"));
    }
    Ok(json!({
        "video": video,
        "poster": poster,
        "width": width,
        "height": height,
        "durationSecs": duration_secs,
        "changed": changed
    }))
}

pub fn place_video(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "video.place";
    let target = opt_id(cmd, p, "target")?;
    if let Some(id) = target {
        let node = s.doc()?.node(id).ok_or(EngineError::NoNode(id))?;
        if !crate::cmd::create::accepts_image_fill(&node.kind) {
            return Err(bad(cmd, "`target` must be a layer that can have a fill"));
        }
    }
    let (bytes, stem) = video_bytes(cmd, p)?;
    let poster_bytes = decode_base64_field(cmd, p, "posterData")?;
    let mode = scale_mode_param(p);
    let autoplay = boolean(p, "autoplay").unwrap_or(true);
    let loop_playback = boolean(p, "loopPlayback").unwrap_or(true);
    let muted = boolean(p, "muted").unwrap_or(true);
    let explicit_name = string(p, "name");

    if let Some(id) = target {
        let doc = s.doc_mut()?;
        let (video, poster, _, _, duration_secs) = register_video_with_poster(cmd, doc, bytes, poster_bytes)?;
        let node = doc.get_mut(id)?;
        let mut vid = VideoPaint::new(video.clone(), Some(poster.clone()), mode);
        vid.autoplay = autoplay;
        vid.loop_playback = loop_playback;
        vid.muted = muted;
        node.fills.push(Paint { kind: PaintKind::Video(vid), ..Paint::solid(figcraft_color::Color::WHITE) });
        node.styles.fill = None;
        return Ok(json!({
            "id": id,
            "video": video,
            "poster": poster,
            "width": node.width,
            "height": node.height,
            "durationSecs": duration_secs
        }));
    }

    let page = s.page()?;
    let parent = opt_id(cmd, p, "parent")?.unwrap_or(page);
    let ds = s.ds_mut()?;
    let doc = ds.doc_mut();
    let (video, poster, vw, vh, duration_secs) = register_video_with_poster(cmd, doc, bytes, poster_bytes)?;
    let (placed_w, placed_h) = figcraft_format::placed_size(vw, vh, 1.0, 4096.0);
    let w = num(p, "width").unwrap_or(placed_w);
    let h = num(p, "height").unwrap_or(if num(p, "width").is_some() {
        w * f64::from(vh) / f64::from(vw).max(1.0)
    } else {
        placed_h
    });
    let nid = doc.alloc_id();
    let name = explicit_name.or(stem).unwrap_or_else(|| "Video".into());
    let mut node = Node::new(nid, NodeKind::Rectangle, name, w, h);
    node.transform = figcraft_geom::Affine::translate((num_or(p, "x", 0.0), num_or(p, "y", 0.0)));
    let mut vid = VideoPaint::new(video.clone(), Some(poster.clone()), mode);
    vid.autoplay = autoplay;
    vid.loop_playback = loop_playback;
    vid.muted = muted;
    node.fills = vec![Paint { kind: PaintKind::Video(vid), ..Paint::solid(figcraft_color::Color::WHITE) }];
    doc.insert(node, parent, None)?;
    ds.selection = vec![nid];
    Ok(json!({
        "id": nid,
        "video": video,
        "poster": poster,
        "width": w,
        "height": h,
        "durationSecs": duration_secs
    }))
}
