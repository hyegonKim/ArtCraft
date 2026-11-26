//! # materialcraft-image
//!
//! Image buffers for node cooking, L0: an [`Image`] is a grayscale (1 channel) or color (RGBA)
//! plane of `f32`, shared copy-on-write through an `Arc` so cached node outputs cost nothing to
//! hand around. Values are linear and usually 0..1 (HDR formats may exceed it). Sampling wraps or
//! clamps per axis ("tiling"); everything per pixel runs in parallel rows (rayon).
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::sync::Arc;

use rayon::prelude::*;

/// Largest side an image may have (an allocation cap for hostile sizes).
pub const MAX_SIDE: u32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Gray,
    Color,
}

impl Kind {
    pub fn channels(self) -> usize {
        match self {
            Kind::Gray => 1,
            Kind::Color => 4,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("image size {0}x{1} is outside 1..={MAX_SIDE}")]
    BadSize(u32, u32),
}

#[derive(Clone, Debug)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub kind: Kind,
    /// Row-major, `kind.channels()` floats per pixel.
    pub data: Arc<Vec<f32>>,
}

impl PartialEq for Image {
    fn eq(&self, o: &Image) -> bool {
        self.w == o.w && self.h == o.h && self.kind == o.kind && (Arc::ptr_eq(&self.data, &o.data) || self.data == o.data)
    }
}

/// Clamp a requested size into the allowed range.
pub fn clamp_size(w: u32, h: u32) -> (u32, u32) {
    (w.clamp(1, MAX_SIDE), h.clamp(1, MAX_SIDE))
}

#[inline]
fn wrap(i: i64, n: u32, tile: bool) -> usize {
    let n = n as i64;
    if tile { i.rem_euclid(n) as usize } else { i.clamp(0, n - 1) as usize }
}

impl Image {
    /// A constant image.
    pub fn filled(w: u32, h: u32, kind: Kind, v: [f32; 4]) -> Image {
        let (w, h) = clamp_size(w, h);
        let n = (w * h) as usize;
        let data = match kind {
            Kind::Gray => vec![v[0]; n],
            Kind::Color => v.iter().copied().cycle().take(n * 4).collect(),
        };
        Image { w, h, kind, data: Arc::new(data) }
    }

    /// Generate every pixel with `f(x, y)` (pixel indices) in parallel rows.
    pub fn generate(w: u32, h: u32, kind: Kind, f: impl Fn(u32, u32) -> [f32; 4] + Sync) -> Image {
        let (w, h) = clamp_size(w, h);
        let c = kind.channels();
        let mut data = vec![0.0f32; (w * h) as usize * c];
        data.par_chunks_mut(w as usize * c).enumerate().for_each(|(y, row)| {
            for x in 0..w as usize {
                let p = f(x as u32, y as u32);
                row[x * c..x * c + c].copy_from_slice(&p[..c]);
            }
        });
        Image { w, h, kind, data: Arc::new(data) }
    }

    /// Generate from normalized coordinates (pixel centers, 0..1).
    pub fn generate_uv(w: u32, h: u32, kind: Kind, f: impl Fn(f32, f32) -> [f32; 4] + Sync) -> Image {
        let (iw, ih) = (1.0 / w.max(1) as f32, 1.0 / h.max(1) as f32);
        Image::generate(w, h, kind, |x, y| f((x as f32 + 0.5) * iw, (y as f32 + 0.5) * ih))
    }

    pub fn gray(w: u32, h: u32, f: impl Fn(u32, u32) -> f32 + Sync) -> Image {
        Image::generate(w, h, Kind::Gray, |x, y| [f(x, y), 0.0, 0.0, 0.0])
    }

    pub fn channels(&self) -> usize {
        self.kind.channels()
    }

    pub fn is_gray(&self) -> bool {
        self.kind == Kind::Gray
    }

    /// The pixel at integer coordinates (wrapped or clamped), as RGBA (gray → `[v, v, v, 1]`).
    #[inline]
    pub fn get(&self, x: i64, y: i64, tile: bool) -> [f32; 4] {
        let (x, y) = (wrap(x, self.w, tile), wrap(y, self.h, tile));
        let i = y * self.w as usize + x;
        match self.kind {
            Kind::Gray => {
                let v = self.data.get(i).copied().unwrap_or(0.0);
                [v, v, v, 1.0]
            }
            Kind::Color => {
                let s = self.data.get(i * 4..i * 4 + 4).unwrap_or(&[0.0, 0.0, 0.0, 1.0]);
                [s[0], s[1], s[2], s[3]]
            }
        }
    }

    /// The gray value, or the Rec. 709 luminance of a color image.
    #[inline]
    pub fn get_gray(&self, x: i64, y: i64, tile: bool) -> f32 {
        let p = self.get(x, y, tile);
        match self.kind {
            Kind::Gray => p[0],
            Kind::Color => luminance(p),
        }
    }

    /// Bilinear sample at normalized `(u, v)`; pixel centers sit at `(i + 0.5) / w`.
    #[inline]
    pub fn sample(&self, u: f32, v: f32, tile: bool) -> [f32; 4] {
        let fx = u * self.w as f32 - 0.5;
        let fy = v * self.h as f32 - 0.5;
        if !fx.is_finite() || !fy.is_finite() {
            return [0.0, 0.0, 0.0, 1.0];
        }
        // Huge but finite coordinates would saturate the i64 cast and overflow at `+ 1`.
        let (fx, fy) = (fx.clamp(-1.0e9, 1.0e9), fy.clamp(-1.0e9, 1.0e9));
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.get(x0, y0, tile);
        let b = self.get(x0 + 1, y0, tile);
        let c = self.get(x0, y0 + 1, tile);
        let d = self.get(x0 + 1, y0 + 1, tile);
        let mut o = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c[k] + (d[k] - c[k]) * tx;
            o[k] = top + (bot - top) * ty;
        }
        o
    }

    #[inline]
    pub fn sample_gray(&self, u: f32, v: f32, tile: bool) -> f32 {
        let p = self.sample(u, v, tile);
        match self.kind {
            Kind::Gray => p[0],
            Kind::Color => luminance(p),
        }
    }

    /// Nearest sample at normalized `(u, v)`.
    #[inline]
    pub fn sample_nearest(&self, u: f32, v: f32, tile: bool) -> [f32; 4] {
        let x = (u * self.w as f32).floor();
        let y = (v * self.h as f32).floor();
        if !x.is_finite() || !y.is_finite() {
            return [0.0, 0.0, 0.0, 1.0];
        }
        self.get(x as i64, y as i64, tile)
    }

    /// Map every pixel (as RGBA) to a new image of `kind`, same size.
    pub fn map(&self, kind: Kind, f: impl Fn([f32; 4]) -> [f32; 4] + Sync) -> Image {
        Image::generate(self.w, self.h, kind, |x, y| f(self.get(x as i64, y as i64, false)))
    }

    /// This image as grayscale (luminance of a color image).
    pub fn to_gray(&self) -> Image {
        match self.kind {
            Kind::Gray => self.clone(),
            Kind::Color => self.map(Kind::Gray, |p| [luminance(p), 0.0, 0.0, 0.0]),
        }
    }

    /// This image as RGBA (gray copied to RGB, alpha 1).
    pub fn to_color(&self) -> Image {
        match self.kind {
            Kind::Color => self.clone(),
            Kind::Gray => self.map(Kind::Color, |p| p),
        }
    }

    pub fn to_kind(&self, kind: Kind) -> Image {
        match kind {
            Kind::Gray => self.to_gray(),
            Kind::Color => self.to_color(),
        }
    }

    /// Resample to `w × h` (bilinear when enlarging, box-filtered when shrinking).
    pub fn resize(&self, w: u32, h: u32, tile: bool) -> Image {
        let (w, h) = clamp_size(w, h);
        if w == self.w && h == self.h {
            return self.clone();
        }
        if w <= self.w && h <= self.h && self.w.is_multiple_of(w) && self.h.is_multiple_of(h) {
            let (fx, fy) = ((self.w / w) as i64, (self.h / h) as i64);
            let inv = 1.0 / (fx * fy) as f32;
            return Image::generate(w, h, self.kind, |x, y| {
                let mut s = [0.0; 4];
                for j in 0..fy {
                    for i in 0..fx {
                        let p = self.get(x as i64 * fx + i, y as i64 * fy + j, tile);
                        for k in 0..4 {
                            s[k] += p[k];
                        }
                    }
                }
                s.map(|v| v * inv)
            });
        }
        Image::generate_uv(w, h, self.kind, |u, v| self.sample(u, v, tile))
    }

    /// Raw float access for gray images (`None` for color).
    pub fn gray_data(&self) -> Option<&[f32]> {
        (self.kind == Kind::Gray).then_some(self.data.as_slice())
    }

    /// 8-bit sRGB-encoded RGBA for display (gray shown as is; color through the sRGB curve).
    pub fn to_rgba8_display(&self) -> Vec<u8> {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        let n = (self.w * self.h) as usize;
        let mut out = vec![0u8; n * 4];
        out.par_chunks_mut(4).enumerate().for_each(|(i, px)| match self.kind {
            Kind::Gray => {
                let v = q(self.data.get(i).copied().unwrap_or(0.0));
                px.copy_from_slice(&[v, v, v, 255]);
            }
            Kind::Color => {
                let s = self.data.get(i * 4..i * 4 + 4).unwrap_or(&[0.0, 0.0, 0.0, 1.0]);
                px.copy_from_slice(&[q(linear_to_srgb(s[0])), q(linear_to_srgb(s[1])), q(linear_to_srgb(s[2])), q(s[3])]);
            }
        });
        out
    }

    /// Mean of each channel (as RGBA), e.g. for tests and auto levels.
    pub fn mean(&self) -> [f32; 4] {
        let c = self.channels();
        let n = (self.w * self.h) as f64;
        let mut s = [0f64; 4];
        for px in self.data.chunks(c) {
            for (k, v) in px.iter().enumerate() {
                s[k] += *v as f64;
            }
        }
        if c == 1 {
            let m = (s[0] / n) as f32;
            return [m, m, m, 1.0];
        }
        s.map(|v| (v / n) as f32)
    }

    /// Minimum and maximum over all channels (alpha excluded).
    pub fn range(&self) -> (f32, f32) {
        let c = self.channels();
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for px in self.data.chunks(c) {
            for v in px.iter().take(c.min(3)) {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
        }
        if lo > hi { (0.0, 0.0) } else { (lo, hi) }
    }
}

/// Rec. 709 luminance of a linear RGB(A) pixel.
#[inline]
pub fn luminance(p: [f32; 4]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

#[inline]
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055 }
}

#[inline]
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Encode the bitmap name and its requested color space without ambiguity.
/// Plain sRGB names remain readable by simple resource callbacks.
pub fn resource_key(name: &str, linear: bool) -> String {
    if linear {
        format!("\u{1}L{name}")
    } else if name.starts_with('\u{1}') {
        format!("\u{1}S{name}")
    } else {
        name.to_string()
    }
}

/// Split a key made by [`resource_key`] into its name and data-map flag.
pub fn split_resource_key(key: &str) -> (&str, bool) {
    if let Some(name) = key.strip_prefix("\u{1}L") {
        (name, true)
    } else if let Some(name) = key.strip_prefix("\u{1}S") {
        (name, false)
    } else {
        (key, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_survives_huge_coordinates() {
        let img = Image::gray(8, 8, |x, _| x as f32);
        for u in [1.0e30_f32, -1.0e30, f32::MAX, f32::MIN] {
            for tile in [true, false] {
                let p = img.sample(u, u, tile);
                assert!(p.iter().all(|c| c.is_finite()));
            }
        }
    }

    #[test]
    fn resource_keys_round_trip() {
        for name in ["n", "", "\u{1}Lnormal", "\u{1}Sn", "n\u{1}linear"] {
            for linear in [false, true] {
                assert_eq!(split_resource_key(&resource_key(name, linear)), (name, linear));
            }
        }
    }

    #[test]
    fn sample_wraps_and_clamps() {
        let img = Image::gray(4, 1, |x, _| x as f32);
        assert_eq!(img.get(-1, 0, true)[0], 3.0);
        assert_eq!(img.get(-1, 0, false)[0], 0.0);
        // Halfway between the last and the (wrapped) first pixel.
        assert!((img.sample(1.0, 0.5, true)[0] - 1.5).abs() < 1e-5);
    }

    #[test]
    fn resize_box_and_bilinear() {
        let img = Image::gray(4, 4, |x, _| if x < 2 { 0.0 } else { 1.0 });
        let small = img.resize(2, 2, true);
        assert_eq!(small.data.as_slice(), &[0.0, 1.0, 0.0, 1.0]);
        let big = small.resize(8, 8, false);
        assert_eq!(big.w, 8);
        assert!(big.data[0] < big.data[7]);
    }

    #[test]
    fn hostile_sizes_are_clamped() {
        let img = Image::filled(0, u32::MAX, Kind::Gray, [0.5; 4]);
        assert_eq!((img.w, img.h), (1, MAX_SIDE));
        assert_eq!(img.sample(f32::NAN, 0.0, true), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn conversions() {
        let c = Image::filled(2, 2, Kind::Color, [1.0, 1.0, 1.0, 1.0]);
        assert!((c.to_gray().data[0] - 1.0).abs() < 1e-5);
        assert_eq!(c.to_rgba8_display()[0], 255);
    }
}
