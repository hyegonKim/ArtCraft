//! Running a filter plug-in over a surface, band by band (WebAssembly) or full-canvas (native `.8bf`).

use photocraft_color::{ColorMode, PixelFormat, SampleType};
use photocraft_geom::{Rect, TILE_SIZE, TileCoord};
use photocraft_raster::Surface;
use serde_json::{Value, json};

use crate::manifest::Area;
use crate::native_vm::{Filter8bfHost, ImageBuffer8bf, PixelDepth8bf};
use crate::runtime::{Deadline, Plugin, native_err};
use crate::{Error, Result};

/// The `format` argument of `pc_filter`: bits 0–7 the source bit depth (8, 16 or 32), bit 8 set
/// when the last channel is (straight) alpha, bits 16–23 the colour mode as numbered in the PSD
/// file format (1 Grayscale, 2 Indexed, 3 RGB, 4 CMYK, 7 Multichannel, 8 Duotone, 9 Lab;
/// 0 Bitmap).
pub fn format_code(f: PixelFormat) -> u32 {
    let depth = match f.sample {
        SampleType::U8 => 8,
        SampleType::U16 => 16,
        SampleType::F32 => 32,
    };
    let mode = match f.mode {
        ColorMode::Bitmap => 0,
        ColorMode::Grayscale => 1,
        ColorMode::Indexed => 2,
        ColorMode::Rgb => 3,
        ColorMode::Cmyk => 4,
        ColorMode::Multichannel => 7,
        ColorMode::Duotone => 8,
        ColorMode::Lab => 9,
    };
    depth | (u32::from(f.alpha) << 8) | (mode << 16)
}

fn mode_name(m: ColorMode) -> &'static str {
    match m {
        ColorMode::Bitmap => "bitmap",
        ColorMode::Grayscale => "grayscale",
        ColorMode::Indexed => "indexed",
        ColorMode::Rgb => "rgb",
        ColorMode::Cmyk => "cmyk",
        ColorMode::Lab => "lab",
        ColorMode::Multichannel => "multichannel",
        ColorMode::Duotone => "duotone",
    }
}

/// Reads `rect`, repeating the edge pixels of `extent` outside it (like built-in neighbourhood
/// filters at the canvas edge).
fn read_clamped(s: &Surface, rect: Rect, extent: Rect) -> Vec<f32> {
    let inner = rect.intersect(&extent);
    if inner.is_empty() || inner == rect {
        return s.read_region(rect);
    }
    let src = s.read_region(inner);
    let n = s.channels();
    let (w, iw) = (rect.width() as usize, inner.width() as usize);
    let mut out = vec![0.0f32; w * rect.height() as usize * n];
    for (row, y) in (rect.y0..rect.y1).enumerate() {
        let sy = (y.clamp(inner.y0, inner.y1 - 1) - inner.y0) as usize;
        for (col, x) in (rect.x0..rect.x1).enumerate() {
            let sx = (x.clamp(inner.x0, inner.x1 - 1) - inner.x0) as usize;
            let s = (sy * iw + sx) * n;
            let d = (row * w + col) * n;
            if let (Some(dst), Some(srcpx)) = (out.get_mut(d..d + n), src.get(s..s + n)) {
                dst.copy_from_slice(srcpx);
            }
        }
    }
    out
}

/// Most bands one run is split into.
const MAX_BANDS: u64 = 1 << 16;

fn pack_rgba_for_8bf(orig: &[f32], channels: usize, pixels: usize) -> Vec<f32> {
    if channels == 4 {
        return orig.to_vec();
    }
    let mut rgba = Vec::with_capacity(pixels * 4);
    for px in orig.chunks_exact(channels.max(1)) {
        match channels {
            1 => rgba.extend_from_slice(&[px[0], px[0], px[0], 1.0]),
            2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 1.0]),
            _ => rgba.extend_from_slice(&[
                px.first().copied().unwrap_or(0.0),
                px.get(1).copied().unwrap_or(0.0),
                px.get(2).copied().unwrap_or(0.0),
                px.get(3).copied().unwrap_or(1.0),
            ]),
        }
    }
    rgba
}

fn unpack_rgba_from_8bf(rgba: &[f32], orig: &[f32], channels: usize, pixels: usize) -> Vec<f32> {
    if channels == 4 {
        return rgba.to_vec();
    }
    let mut out = vec![0.0f32; pixels * channels];
    for (idx, rgba_px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let base = idx * channels;
        match channels {
            1 => {
                if let Some(d) = out.get_mut(base) {
                    *d = rgba_px[0];
                }
            }
            2 => {
                if let Some(d) = out.get_mut(base..base + 2) {
                    d[0] = rgba_px[0];
                    d[1] = rgba_px[3];
                }
            }
            3 => {
                if let Some(d) = out.get_mut(base..base + 3) {
                    d.copy_from_slice(&rgba_px[..3]);
                }
            }
            _ => {
                for ch in 0..channels {
                    let v = rgba_px.get(ch).copied().unwrap_or_else(|| orig.get(base + ch).copied().unwrap_or(0.0));
                    if let Some(d) = out.get_mut(base + ch) {
                        *d = v;
                    }
                }
            }
        }
    }
    out
}

impl Plugin {
    /// The rectangle a run writes: the layer's content (grown by the plug-in's overlap) or the
    /// canvas, limited to the selection's bounds.
    pub fn output_area(&self, surface: &Surface, canvas: Rect, selection: Option<&Surface>) -> Rect {
        let content = surface.content_bounds();
        let extent = canvas.union(&content);
        let mut area = match self.manifest().area {
            Area::Content if content.is_empty() => Rect::EMPTY,
            Area::Content => content.inflate(self.manifest().overlap as i32).intersect(&extent),
            Area::Canvas => canvas,
        };
        if let Some(sel) = selection {
            area = area.intersect(&sel.content_bounds());
        }
        area
    }

    /// Runs the filter on `surface` with user `params` (validated against the manifest) and
    /// returns the new surface. `canvas` is the document bounds; `selection` (coverage in
    /// channel 0) limits and feathers the result, as for built-in filters.
    pub fn apply(&self, surface: &Surface, canvas: Rect, selection: Option<&Surface>, params: &Value) -> Result<Surface> {
        let resolved = self.manifest().resolve_params(params)?;
        let area = self.output_area(surface, canvas, selection);
        let mut out = surface.clone();
        if area.is_empty() {
            return Ok(out);
        }
        let extent = canvas.union(&surface.content_bounds());
        if let Some(host) = self.native_8bf_host() {
            return self.apply_native_8bf(host, surface, area, extent, selection);
        }
        let fmt = surface.format();
        let n = fmt.channels();
        let ov = self.manifest().overlap as i32;
        // Bands are tile-aligned blocks, one tile row tall and as many tiles wide as fit in
        // `band_bytes`, so each worker can encode its own tiles and the merge is a tile copy.
        let tile_bytes = (TILE_SIZE as usize).pow(2) * n * 4;
        // Half the plug-in's memory cap is for the band (the module needs room of its own).
        let room = self.limits().max_memory_bytes / 2;
        let across = (self.limits().band_bytes.min(room) / tile_bytes).clamp(1, 1024) as i32;
        let in_bytes = ((across * TILE_SIZE + 2 * ov) as usize) * ((TILE_SIZE + 2 * ov) as usize) * n * 4;
        if in_bytes > room {
            return Err(Error::Limit("memory budget (a band does not fit in the plug-in's memory)".into()));
        }
        let mut bands = Vec::new();
        let (t0, t1) = (TileCoord::containing(area.x0, area.y0), TileCoord::containing(area.x1 - 1, area.y1 - 1));
        // Counted before any is made: a few distant tiles make `area` a grid of millions of
        // bands, each a call into the plug-in, planned before the deadline starts.
        let columns = (i64::from(t1.tx) - i64::from(t0.tx) + 1).max(0) as u64;
        let rows = (i64::from(t1.ty) - i64::from(t0.ty) + 1).max(0) as u64;
        if columns.div_ceil(across as u64).saturating_mul(rows) > MAX_BANDS {
            return Err(Error::Limit(format!("more than {MAX_BANDS} bands (the layer's content is spread too widely)")));
        }
        for ty in t0.ty..=t1.ty {
            let mut tx = t0.tx;
            while tx <= t1.tx {
                let r = Rect::new(tx * TILE_SIZE, ty * TILE_SIZE, (tx + across).min(t1.tx + 1) * TILE_SIZE, (ty + 1) * TILE_SIZE);
                bands.push(r.intersect(&area));
                tx += across;
            }
        }
        let deadline = Deadline::new(self.limits());
        let ctx = BandCtx { surface, extent, canvas, selection, fmt, ov, params: Value::Object(resolved) };
        let run = |b: &Rect| {
            let r = self.run_band(&ctx, *b, &deadline).map(|data| encode_band(surface, *b, &data));
            if let Err(error) = &r {
                deadline.abort(error.clone());
            }
            r
        };
        // A few bands at a time: one per worker, and no more instances than the run's memory
        // budget holds at their cap, so peak memory stays bounded however many cores run (#8603).
        #[cfg(not(target_arch = "wasm32"))]
        let group = self.limits().parallel_bands(rayon::current_num_threads());
        #[cfg(target_arch = "wasm32")]
        let group = 1;
        for chunk in bands.chunks(group) {
            #[cfg(not(target_arch = "wasm32"))]
            let results: Vec<Result<Surface>> = {
                use rayon::prelude::*;
                chunk.par_iter().map(run).collect()
            };
            #[cfg(target_arch = "wasm32")]
            let results: Vec<Result<Surface>> = chunk.iter().map(run).collect();
            for r in results {
                for (c, t) in r?.tiles() {
                    out.tile_mut(*c).bytes_mut().copy_from_slice(&t.bytes());
                }
            }
        }
        out.prune();
        Ok(out)
    }

    fn apply_native_8bf(&self, host: &Filter8bfHost, surface: &Surface, area: Rect, extent: Rect, selection: Option<&Surface>) -> Result<Surface> {
        let deadline = Deadline::new(self.limits());
        deadline.check()?;
        let fmt = surface.format();
        let n = fmt.channels();
        let (w, h) = (area.width(), area.height());
        let pixels = (w as usize).checked_mul(h as usize).ok_or_else(|| Error::Limit("memory budget (image dimensions overflow)".into()))?;
        let depth = match fmt.sample {
            SampleType::U8 => PixelDepth8bf::U8,
            SampleType::U16 => PixelDepth8bf::U16,
            SampleType::F32 => PixelDepth8bf::F32,
        };
        let staging_bytes = pixels
            .checked_mul(4)
            .and_then(|s| s.checked_mul(depth.bytes_per_sample()))
            .and_then(|b| b.checked_mul(2))
            .ok_or_else(|| Error::Limit("memory budget (staging buffer overflow)".into()))?;
        if staging_bytes > self.limits().max_memory_bytes / 2 {
            return Err(Error::Limit("memory budget (image does not fit in the plug-in's memory)".into()));
        }

        let orig = read_clamped(surface, area, extent);
        let rgba_in = pack_rgba_for_8bf(&orig, n, pixels);
        let img_buf = ImageBuffer8bf::from_rgba_f32(w, h, rgba_in, depth).map_err(native_err)?;
        let exec_limits = crate::native_vm::Limits {
            max_binary_bytes: self.limits().max_module_bytes,
            max_memory_bytes: self.limits().max_memory_bytes,
            instruction_budget: self.limits().fuel_base.saturating_add((pixels as u64).saturating_mul(4).saturating_mul(self.limits().fuel_per_sample)),
            max_call_depth: self.limits().max_recursion_depth.clamp(16, 512),
        };
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| host.execute_with_limits(&img_buf, exec_limits)));
        let filtered_rgba = run.map_err(|_| Error::Trap("the runtime panicked".into()))?.map_err(native_err)?;
        deadline.check()?;

        let filtered = unpack_rgba_from_8bf(&filtered_rgba.samples, &orig, n, pixels);
        let (bw, bh) = (w as usize, h as usize);
        let mut blended = vec![0.0f32; bw * bh * n];
        let sel_row = selection.map(|s| s.format().channels());
        let mut sel = Vec::new();
        let int_depth = fmt.sample != SampleType::F32;
        for row in 0..bh {
            if let (Some(s), Some(sn)) = (selection, sel_row) {
                let r = Rect::new(area.x0, area.y0 + row as i32, area.x1, area.y0 + row as i32 + 1);
                sel = s.read_region(r).chunks_exact(sn.max(1)).map(|p| p.first().copied().unwrap_or(0.0).clamp(0.0, 1.0)).collect();
            }
            for col in 0..bw {
                let idx = (row * bw + col) * n;
                let k = if selection.is_some() { sel.get(col).copied().unwrap_or(0.0) } else { 1.0 };
                for ch in 0..n {
                    let o = orig.get(idx + ch).copied().unwrap_or(0.0);
                    let mut v = filtered.get(idx + ch).copied().unwrap_or(o);
                    if !v.is_finite() {
                        v = o;
                    }
                    if int_depth || (fmt.alpha && ch == n - 1) {
                        v = v.clamp(0.0, 1.0);
                    }
                    if let Some(d) = blended.get_mut(idx + ch) {
                        *d = o * (1.0 - k) + v * k;
                    }
                }
            }
        }

        let mut out = surface.clone();
        out.write_region(area, &blended);
        out.prune();
        Ok(out)
    }

    fn run_band(&self, c: &BandCtx, band: Rect, deadline: &Deadline) -> Result<Vec<f32>> {
        let n = c.fmt.channels();
        let input = band.inflate(c.ov);
        let mut data = read_clamped(c.surface, input, c.extent);
        let (iw, ih) = (input.width(), input.height());
        let mut params = c.params.clone();
        if let Value::Object(m) = &mut params {
            m.insert(
                "_image".into(),
                json!({
                    "x": input.x0, "y": input.y0, "width": iw, "height": ih, "overlap": c.ov,
                    "canvasWidth": c.canvas.width(), "canvasHeight": c.canvas.height(),
                    "mode": mode_name(c.fmt.mode), "depth": format_code(c.fmt) & 0xff, "alpha": c.fmt.alpha, "channels": n,
                }),
            );
        }
        let pbytes = serde_json::to_vec(&params).map_err(|e| Error::Params(e.to_string()))?;
        let orig_full = data.clone();
        let run =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.filter_buffer(&mut data, iw, ih, n as u32, format_code(c.fmt), &pbytes, deadline)));
        run.map_err(|_| Error::Trap("the runtime panicked".into()))??;
        // Keep the band (drop the overlap margins), sanitise and blend by the selection.
        let (bw, bh) = (band.width() as usize, band.height() as usize);
        let ov = c.ov as usize;
        let mut out = vec![0.0f32; bw * bh * n];
        let sel_row = c.selection.map(|s| s.format().channels());
        let mut sel = Vec::new();
        let int_depth = c.fmt.sample != SampleType::F32;
        for row in 0..bh {
            if let (Some(s), Some(sn)) = (c.selection, sel_row) {
                let r = Rect::new(band.x0, band.y0 + row as i32, band.x1, band.y0 + row as i32 + 1);
                sel = s.read_region(r).chunks_exact(sn.max(1)).map(|p| p.first().copied().unwrap_or(0.0).clamp(0.0, 1.0)).collect();
            }
            for col in 0..bw {
                let si = ((row + ov) * (bw + 2 * ov) + col + ov) * n;
                let di = (row * bw + col) * n;
                let k = if c.selection.is_some() { sel.get(col).copied().unwrap_or(0.0) } else { 1.0 };
                for ch in 0..n {
                    let o = orig_full.get(si + ch).copied().unwrap_or(0.0);
                    let mut v = data.get(si + ch).copied().unwrap_or(o);
                    if !v.is_finite() {
                        v = o;
                    }
                    if int_depth || (c.fmt.alpha && ch == n - 1) {
                        v = v.clamp(0.0, 1.0);
                    }
                    if let Some(d) = out.get_mut(di + ch) {
                        // As a built-in edit mixes through the selection (#9483).
                        *d = o * (1.0 - k) + v * k;
                    }
                }
            }
        }
        Ok(out)
    }
}

/// The tiles under `band` with the band's new pixels written in (the rest of each tile is the
/// original), encoded in the surface's format: built on the worker thread.
fn encode_band(surface: &Surface, band: Rect, data: &[f32]) -> Surface {
    let mut s = Surface::with_default(surface.format(), &surface.default_pixel());
    for c in band.tiles() {
        if let Some(t) = surface.tile(c) {
            s.tile_mut(c).bytes_mut().copy_from_slice(&t.bytes());
        } else {
            s.tile_mut(c);
        }
    }
    s.write_region(band, data);
    s
}

struct BandCtx<'a> {
    surface: &'a Surface,
    extent: Rect,
    canvas: Rect,
    selection: Option<&'a Surface>,
    fmt: PixelFormat,
    ov: i32,
    params: Value,
}
