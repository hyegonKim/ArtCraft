use super::paths::{join, read_file, sanitize, stem, write_file};
use super::save::{SaveOpts, encode};
use super::{ColorMode, Document, EngineError, Layer, LayerContent, LayerId, Rect, Result, Session, Surface, Value, from_rgba_into, json, str_param};

fn lut_apply_trilinear(lut: &craft_cms::lutfile::LutFile, rgb: [f32; 3]) -> [f32; 3] {
    let n = lut.size.max(2);
    if lut.data.len() < n * n * n * 3 {
        return rgb;
    }
    let s = (n - 1) as f32;
    let mut idx = [0usize; 3];
    let mut frac = [0.0f32; 3];
    for k in 0..3 {
        let span = (lut.domain_max[k] - lut.domain_min[k]).max(1e-6);
        let x = (((rgb[k] - lut.domain_min[k]) / span).clamp(0.0, 1.0)) * s;
        let i = (x.floor() as usize).min(n - 2);
        idx[k] = i;
        frac[k] = x - i as f32;
    }
    let at = |r: usize, g: usize, b: usize| {
        let base = ((b * n + g) * n + r) * 3;
        [lut.data[base], lut.data[base + 1], lut.data[base + 2]]
    };
    let lerp = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let [r, g, b] = idx;
    let c00 = lerp(at(r, g, b), at(r + 1, g, b), frac[0]);
    let c10 = lerp(at(r, g + 1, b), at(r + 1, g + 1, b), frac[0]);
    let c01 = lerp(at(r, g, b + 1), at(r + 1, g, b + 1), frac[0]);
    let c11 = lerp(at(r, g + 1, b + 1), at(r + 1, g + 1, b + 1), frac[0]);
    lerp(lerp(c00, c10, frac[1]), lerp(c01, c11, frac[1]), frac[2])
}

fn lut_chain(first: &craft_cms::lutfile::LutFile, next: &craft_cms::lutfile::LutFile, size: usize, title: &str) -> Result<craft_cms::lutfile::LutFile> {
    craft_cms::lutfile::LutFile::from_fn(title, size.max(2), |c| lut_apply_trilinear(next, lut_apply_trilinear(first, c)))
        .map_err(|e| EngineError::Other(e.to_string()))
}

fn lut_with_intensity(lut: &craft_cms::lutfile::LutFile, amount: f32) -> Result<craft_cms::lutfile::LutFile> {
    let t = if amount.is_finite() { amount.clamp(0.0, 1.0) } else { 1.0 };
    craft_cms::lutfile::LutFile::from_fn(&lut.title, lut.size.max(2), |c| {
        let out = lut_apply_trilinear(lut, c);
        [c[0] + (out[0] - c[0]) * t, c[1] + (out[1] - c[1]) * t, c[2] + (out[2] - c[2]) * t]
    })
    .map_err(|e| EngineError::Other(e.to_string()))
}

// ---------- Export ----------

pub(super) fn layers_to_files(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "file.export.layersToFiles";
    let dir = str_param(p, "dir", cmd).or_else(|_| str_param(p, "output", cmd))?.to_string();
    let report = export_layers_report_with(s, p, |name, bytes| write_file(&join(&dir, name), bytes))?;
    let files: Vec<String> = report.files.iter().map(|name| join(&dir, name)).collect();
    Ok(json!({"files": files, "skipped": report.skipped}))
}

/// The bodies of `file.export.layersToFiles`, streamed to `write` as a single-component filename
/// plus its bytes: one file per exported layer (top to bottom, like Photoshop's numbering),
/// encoded but never touched as a path. Automation writes exactly these bytes beneath its write
/// root (#3216), so the two paths cannot drift. Returns the filenames written, in order.
pub fn export_layers_with(s: &Session, p: &Value, write: impl FnMut(&str, &[u8]) -> Result<()>) -> Result<Vec<String>> {
    export_layers_report_with(s, p, write).map(|report| report.files)
}

/// A top-level layer omitted from a successful Layers to Files export.
#[derive(Debug, serde::Serialize)]
pub struct SkippedExportLayer {
    /// Session layer id (names need not be unique).
    pub layer: u64,
    /// Original name, not the sanitized filename component.
    pub name: String,
    /// Why the layer produced no file.
    pub reason: &'static str,
}

/// Filenames written and layers skipped, each in top-to-bottom order.
#[derive(Debug, Default)]
pub struct LayerExportReport {
    pub files: Vec<String>,
    pub skipped: Vec<SkippedExportLayer>,
}

/// The same export as [`export_layers_with`], with an explanation for every omitted top-level
/// layer. Groups export merged, so their children are not separate exports or skips. Encoding
/// and write failures remain errors, never successful skips. Existing numbering is preserved.
pub fn export_layers_report_with(s: &Session, p: &Value, mut write: impl FnMut(&str, &[u8]) -> Result<()>) -> Result<LayerExportReport> {
    let cmd = "file.export.layersToFiles";
    let format = p.get("format").and_then(Value::as_str).unwrap_or("png").trim_start_matches('.').to_ascii_lowercase();
    // The format becomes the file extension inside one filename component handed to `write`.
    if format.is_empty() || !format.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(EngineError::BadParams { cmd: cmd.into(), msg: "`format` must be a file extension such as png".into() });
    }
    let trim = p.get("trim").and_then(Value::as_bool).unwrap_or(false);
    let visible_only = p.get("visibleOnly").and_then(Value::as_bool).unwrap_or(true);
    // Callers outside `Session::execute` (headless workspace exports) carry the session's gate
    // into `encode`, so a PSD embeds no file a linked smart object names.
    let _authorization = crate::jobs::render_authorization_scope(s.authorize);
    let save = SaveOpts::from_params(s, p, cmd)?;
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let prefix = p.get("prefix").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| stem(&d.doc.name));
    let doc = d.doc.clone();
    let mut report = LayerExportReport::default();
    // Hidden layers never consumed a filename index; adjustments that pass the visibility
    // filter did. Keep those filenames stable while reporting both kinds of omission.
    let mut i = 0usize;
    for l in doc.layers.iter().rev() {
        if !l.visible && visible_only {
            report.skipped.push(SkippedExportLayer { layer: l.id.0, name: l.name.clone(), reason: "hidden by visibleOnly" });
            continue;
        }
        let index = i;
        i += 1;
        if matches!(l.content, LayerContent::Adjustment(_)) {
            report.skipped.push(SkippedExportLayer { layer: l.id.0, name: l.name.clone(), reason: "adjustment layer has no pixels of its own" });
            continue;
        }
        let mut one = (*doc).clone();
        let mut only = l.clone();
        only.visible = true;
        only.clipped = false;
        one.layers = vec![only];
        if trim {
            let mut bounds = Rect::EMPTY;
            let _ = photocraft_compose::render_bands(&one, one.bounds(), 0, |band| -> std::result::Result<(), ()> {
                let width = band.rect.width() as usize;
                for (i, rgba) in band.px.iter().enumerate() {
                    if rgba[3] > 0.0 && width > 0 {
                        let x = band.rect.x0 + (i % width) as i32;
                        let y = band.rect.y0 + (i / width) as i32;
                        bounds = bounds.union(&Rect::new(x, y, x + 1, y + 1));
                    }
                }
                Ok(())
            });
            if !bounds.is_empty() && bounds != one.bounds() {
                crate::image_cmds::crop_doc(&mut one, cmd, bounds, true, [0.0; 4])?;
            }
        }

        let name = format!("{}_{:04}_{}.{format}", sanitize(&prefix), index, sanitize(&l.name));
        let (bytes, _warnings) = encode(&one, &name, save)?;
        write(&name, &bytes)?;
        report.files.push(name);
    }
    Ok(report)
}

/// Adjustment layers that can be represented as an RGB-only LUT, in document stack order.
/// A LUT has no spatial coordinates, so masks and clipping geometry are deliberately stripped
/// at bake time. Group adjustments need their enclosing group's compositing semantics and are
/// not silently flattened as independent layers.
fn lut_layers<'a>(doc: &'a Document, ids: Option<&[LayerId]>) -> Result<Vec<&'a Layer>> {
    const CMD: &str = "file.export.colorLookupTables";
    let mut selected = Vec::new();
    if let Some(ids) = ids {
        if ids.is_empty() {
            return Err(EngineError::BadParams { cmd: CMD.into(), msg: "select at least one adjustment layer".into() });
        }
        for id in ids {
            if selected.contains(id) {
                return Err(EngineError::BadParams { cmd: CMD.into(), msg: format!("duplicate layer id {}", id.0) });
            }
            let Some(layer) = doc.layer(*id) else { return Err(EngineError::NoLayer(*id)) };
            if !matches!(layer.content, LayerContent::Adjustment(_)) {
                return Err(EngineError::BadParams { cmd: CMD.into(), msg: format!("layer {} is not an adjustment layer", id.0) });
            }
            if !layer.visible {
                return Err(EngineError::BadParams { cmd: CMD.into(), msg: format!("layer {} is hidden", id.0) });
            }
            if !doc.layers.iter().any(|root| root.id == *id) {
                return Err(EngineError::BadParams {
                    cmd: CMD.into(),
                    msg: format!("layer {} is nested in a group; export top-level adjustment layers", id.0),
                });
            }
            selected.push(*id);
        }
    }
    let mut layers = Vec::new();
    for l in &doc.layers {
        if l.visible && matches!(l.content, LayerContent::Adjustment(_)) && (ids.is_none() || selected.contains(&l.id)) {
            layers.push(l);
        }
    }
    if layers.is_empty() {
        return Err(EngineError::BadParams { cmd: CMD.into(), msg: "no visible adjustment layers to export".into() });
    }
    Ok(layers)
}

/// Bake the chosen adjustment stack against an identity RGB lattice.
/// Source order always follows the document stack, even if an explicit list is reversed.
fn bake_cube_layers(size: usize, title: &str, layers: &[&Layer], depth: photocraft_color::SampleType) -> String {
    let n = size;
    let (w, h) = ((n * n) as u32, n as u32);
    let mut lattice = Document::new("lut", photocraft_doc::Size::new(w, h), ColorMode::Rgb, photocraft_color::SampleType::F32);
    let fmt = lattice.pixel_format();
    let step = 1.0 / (n - 1) as f32;
    let mut data = Vec::with_capacity((w * h) as usize * fmt.channels());
    for g in 0..n {
        for b in 0..n {
            for r in 0..n {
                let mut px = [0.0f32; 8];
                let k = from_rgba_into(&fmt, [r as f32 * step, g as f32 * step, b as f32 * step, 1.0], &mut px);
                data.extend_from_slice(&px[..k]);
            }
        }
    }
    let mut surf = Surface::new(fmt);
    surf.write_region(Rect::new(0, 0, w as i32, h as i32), &data);
    lattice.layers.push(Layer::new("Lattice", LayerContent::Raster(surf)));
    for l in layers {
        let mut a = (*l).clone();
        // Masks and clipping are spatial; a LUT is the adjustment stack's colour mapping.
        a.mask = None;
        a.vector_mask = None;
        a.clipped = false;
        lattice.layers.push(a);
    }
    let out = photocraft_compose::flatten(&lattice);
    // The reference application's comment layout. There is no Copyright input yet (#13748), so the
    // `#Copyright:` line is never written.
    let mut s = format!(
        "#Created by: PhotoCraft\nTITLE \"{}\"\n\n#LUT size\nLUT_3D_SIZE {n}\n\n#data domain\nDOMAIN_MIN 0.0 0.0 0.0\nDOMAIN_MAX 1.0 1.0 1.0\n\n#LUT data points\n",
        title.replace('"', "'")
    );
    // 8- and 16-bit documents write values on 1/32768 steps; 32-bit stays unquantised.
    let quantise = |v: f32| -> f32 {
        let v = v.clamp(0.0, 1.0);
        if matches!(depth, photocraft_color::SampleType::F32) { v } else { (v * 32768.0).round() / 32768.0 }
    };
    // .cube order: red changes fastest, then green, then blue.
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let (x, y) = (b * n + r, g);
                let px = out.px[y * w as usize + x];
                s.push_str(&format!("{:.6} {:.6} {:.6}\n", quantise(px[0]), quantise(px[1]), quantise(px[2])));
            }
        }
    }
    s
}

/// Backwards-compatible helper for exporting all visible top-level adjustments.
pub fn bake_cube(doc: &Document, size: usize, title: &str) -> String {
    let layers: Vec<&Layer> = doc.layers.iter().filter(|l| l.visible && matches!(l.content, LayerContent::Adjustment(_))).collect();
    bake_cube_layers(size.clamp(2, craft_cms::lutfile::MAX_SIZE), title, &layers, doc.depth)
}

/// Export either the whole visible stack (backward-compatible default), the Layers panel's
/// current selection, or an explicit list supplied by scripts/CLI/automation.
pub(super) fn color_lookup_tables(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.export.colorLookupTables";
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let size = match p.get("size") {
        None | Some(Value::Null) => 33,
        Some(v) => v.as_u64().ok_or_else(|| EngineError::BadParams { cmd: CMD.into(), msg: "size must be an integer".into() })?,
    };
    if !(2..=craft_cms::lutfile::MAX_SIZE as u64).contains(&size) {
        return Err(EngineError::BadParams { cmd: CMD.into(), msg: format!("size must be 2..={}", craft_cms::lutfile::MAX_SIZE) });
    }
    let scope = match p.get("scope") {
        None | Some(Value::Null) => "all",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err(EngineError::BadParams { cmd: CMD.into(), msg: "scope must be all or selected".into() }),
    };
    if !matches!(scope, "all" | "selected") {
        return Err(EngineError::BadParams { cmd: CMD.into(), msg: "scope must be all or selected".into() });
    }
    if p.get("layers").is_some() && scope == "selected" {
        return Err(EngineError::BadParams { cmd: CMD.into(), msg: "use either layers or scope=selected, not both".into() });
    }
    let explicit: Option<Vec<LayerId>> = if let Some(v) = p.get("layers") {
        let Some(a) = v.as_array() else { return Err(EngineError::BadParams { cmd: CMD.into(), msg: "layers must be an array of layer IDs".into() }) };
        let mut out = Vec::with_capacity(a.len());
        for id in a {
            let Some(id) = id.as_u64() else { return Err(EngineError::BadParams { cmd: CMD.into(), msg: "layer IDs must be unsigned integers".into() }) };
            out.push(LayerId(id));
        }
        Some(out)
    } else if scope == "selected" {
        Some(d.selected_layers())
    } else {
        None
    };
    let layers = lut_layers(&d.doc, explicit.as_deref())?;
    let count = layers.len();
    let title = p.get("title").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| stem(&d.doc.name));
    let mut cube = bake_cube_layers(size as usize, &title, &layers, d.doc.depth);
    if p.get("intensity").is_some() || p.get("chainPath").is_some() {
        let mut lut = craft_cms::lutfile::parse_cube(&cube).map_err(|e| EngineError::Other(e.to_string()))?;
        if let Some(chain_path) = p.get("chainPath").and_then(Value::as_str) {
            let bytes = read_file(chain_path)?;
            let next = craft_cms::lutfile::parse(chain_path, &bytes)
                .map_err(|e| EngineError::BadParams { cmd: CMD.into(), msg: format!("chainPath: {e}") })?;
            lut = lut_chain(&lut, &next, size as usize, &title)?;
        }
        if let Some(intensity) = p.get("intensity").and_then(Value::as_f64) {
            lut = lut_with_intensity(&lut, intensity as f32)?;
        }
        cube = craft_cms::lutfile::write_cube(&lut);
    }
    match p.get("path").and_then(Value::as_str) {
        Some(path) => {
            write_file(path, cube.as_bytes())?;
            Ok(json!({"path": path, "size": size, "layerCount": count}))
        }
        None => Ok(json!({"cube": cube, "size": size, "layerCount": count})),
    }
}

pub(super) fn motion_project(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "file.export.motionProject";
    let d = s.active().ok_or(EngineError::NoDocument)?;
    let mut opts = photocraft_doc::motion::MotionExportOptions::default();
    if let Some(fps) = p.get("frameRate").and_then(Value::as_f64) {
        if !fps.is_finite() || !(1.0..=240.0).contains(&fps) {
            return Err(EngineError::BadParams { cmd: CMD.into(), msg: "frameRate must be 1..=240".into() });
        }
        opts.fps = fps as f32;
    }
    if let Some(dur) = p.get("durationFrames").and_then(Value::as_u64) {
        if dur == 0 {
            return Err(EngineError::BadParams { cmd: CMD.into(), msg: "durationFrames must be >= 1".into() });
        }
        opts.duration_frames = dur as usize;
    }
    if let Some(pub_ctrl) = p.get("publishParameters").and_then(Value::as_bool) {
        opts.publish_template_controls = pub_ctrl;
    }
    let ozml = photocraft_doc::motion::export_motion_ozml(&d.doc, &opts);
    match p.get("path").and_then(Value::as_str) {
        Some(path) => {
            write_file(path, ozml.as_bytes())?;
            Ok(json!({"path": path, "bytes": ozml.len(), "layerCount": d.doc.layers.len()}))
        }
        None => Ok(json!({"ozml": ozml, "bytes": ozml.len(), "layerCount": d.doc.layers.len()})),
    }
}

// ---------- guides ----------

pub(super) fn push_unique(v: &mut Vec<f64>, x: f64) {
    if !v.iter().any(|g| (g - x).abs() < 0.01) {
        v.push(x);
    }
}
