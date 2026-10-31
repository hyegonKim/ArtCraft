//! FFT magnitude analysis for analyzers and EQ displays.

use rustfft::{FftPlanner, num_complex::Complex};

/// Largest FFT size accepted (larger requests are clamped).
pub const MAX_FFT: usize = 1 << 18;

/// Hann-windowed magnitude spectrum in dBFS, `fft_size / 2 + 1` bins (bin `k` is at
/// `k · sr / fft_size` Hz). A full-scale sine reads ≈ 0 dB. Input shorter than `fft_size` is
/// zero-padded; longer input uses the first `fft_size` samples. Floored at -144 dB.
pub fn magnitude_db(samples: &[f32], fft_size: usize) -> Vec<f32> {
    let n = fft_size.min(MAX_FFT);
    if n < 2 {
        return Vec::new();
    }
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut buf: Vec<Complex<f32>> = (0..n)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
            let x = samples.get(i).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
            Complex::new(x * w, 0.0)
        })
        .collect();
    fft.process(&mut buf);
    // Hann coherent gain is 0.5; a sine's energy splits across ±f.
    let scale = 2.0 / (n as f32 * 0.5);
    buf.iter().take(n / 2 + 1).map(|c| crate::gain_to_db(c.norm() * scale)).collect()
}

/// A Hann-windowed magnitude analyzer that keeps one FFT plan and its buffers, for drawing many
/// spectra (spectrogram columns) without planning or allocating per spectrum. Levels are scaled
/// as [`magnitude_db`]'s: a full-scale sine reads about 0 dB.
pub struct SpectrumAnalyzer {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl SpectrumAnalyzer {
    /// Smallest and largest FFT sizes the analyzer accepts (others are clamped).
    pub const SIZES: (usize, usize) = (16, 1 << 14);

    pub fn new(fft_size: usize) -> Self {
        let n = fft_size.clamp(Self::SIZES.0, Self::SIZES.1);
        let fft = FftPlanner::<f32>::new().plan_fft_forward(n);
        let window = (0..n).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()).collect();
        let scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        Self { fft, window, buf: vec![Complex::new(0.0, 0.0); n], scratch }
    }

    /// The FFT size.
    pub fn size(&self) -> usize {
        self.window.len()
    }

    /// The `size() / 2 + 1` bin levels (dBFS) of `samples[start..start + size()]`, zero-padded past
    /// the end (non-finite samples count as silence), written to `out`.
    pub fn magnitude_db(&mut self, samples: &[f32], start: usize, out: &mut Vec<f32>) {
        let n = self.size();
        for (i, (slot, w)) in self.buf.iter_mut().zip(&self.window).enumerate() {
            let x = start.checked_add(i).and_then(|k| samples.get(k)).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
            *slot = Complex::new(x * w, 0.0);
        }
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);
        // Hann coherent gain is 0.5; a sine's energy splits across ±f.
        let scale = 2.0 / (n as f32 * 0.5);
        out.clear();
        out.extend(self.buf.iter().take(n / 2 + 1).map(|c| crate::gain_to_db(c.norm() * scale)));
    }
}

/// Most columns [`spectrogram_db`] returns.
pub const MAX_SPECTROGRAM_COLUMNS: usize = 2048;

/// An STFT spectrogram: columns of `fft_size / 2 + 1` bin levels (dBFS), one every `hop_size`
/// samples, or further apart so there are at most [`MAX_SPECTROGRAM_COLUMNS`].
pub fn spectrogram_db(samples: &[f32], fft_size: usize, hop_size: usize) -> Vec<Vec<f32>> {
    if samples.is_empty() {
        return Vec::new();
    }
    let hop = hop_size.max(1);
    let columns = samples.len().div_ceil(hop);
    let step = hop.saturating_mul(columns.div_ceil(MAX_SPECTROGRAM_COLUMNS).max(1));
    let mut analyzer = SpectrumAnalyzer::new(fft_size);
    let mut out = Vec::with_capacity(samples.len().div_ceil(step).min(MAX_SPECTROGRAM_COLUMNS));
    let mut start = 0usize;
    while start < samples.len() && out.len() < MAX_SPECTROGRAM_COLUMNS {
        let mut col = Vec::new();
        analyzer.magnitude_db(samples, start, &mut col);
        out.push(col);
        start = start.saturating_add(step);
    }
    out
}

/// Maps a dBFS level (`floor_db..ceiling_db`, e.g. `-90.0..0.0`) to an RGB colour on a
/// magma-like ramp (black, purple, red-orange, pale yellow), for the spectral display. The stops
/// are samples of matplotlib's "magma" colour map (CC0).
pub fn magma_color(db: f32, floor_db: f32, ceiling_db: f32) -> [u8; 3] {
    let span = (ceiling_db - floor_db).max(1.0);
    let t = if db.is_nan() { 0.0 } else { ((db - floor_db) / span).clamp(0.0, 1.0) };
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.00, [0.0, 0.0, 4.0]),
        (0.25, [81.0, 18.0, 124.0]),
        (0.50, [183.0, 55.0, 121.0]),
        (0.75, [252.0, 137.0, 97.0]),
        (1.00, [252.0, 253.0, 191.0]),
    ];
    for pair in STOPS.windows(2) {
        if let [(t0, c0), (t1, c1)] = *pair
            && t <= t1
        {
            let u = ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
            return [
                (c0[0] + (c1[0] - c0[0]) * u).round() as u8,
                (c0[1] + (c1[1] - c0[1]) * u).round() as u8,
                (c0[2] + (c1[2] - c0[2]) * u).round() as u8,
            ];
        }
    }
    [252, 253, 191]
}

/// Frequency in Hz of bin `k` for a given FFT size.
pub fn bin_hz(k: usize, fft_size: usize, sample_rate: f32) -> f32 {
    if fft_size == 0 { 0.0 } else { k as f32 * sample_rate / fft_size as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_peak_near_zero_db() {
        let sr = 48_000.0;
        let n = 4096;
        let k = 100;
        let f = bin_hz(k, n, sr);
        let x: Vec<f32> = (0..n).map(|i| (std::f32::consts::TAU * f * i as f32 / sr).sin()).collect();
        let m = magnitude_db(&x, n);
        assert_eq!(m.len(), n / 2 + 1);
        assert!(m[k].abs() < 0.1, "{}", m[k]);
        assert!(magnitude_db(&x, 0).is_empty());
        let spec = spectrogram_db(&x, 256, 128);
        assert!(!spec.is_empty());
        assert_eq!(spec[0].len(), 129);
        // The analyzer reads the same levels as magnitude_db.
        let mut a = SpectrumAnalyzer::new(n);
        let mut col = Vec::new();
        a.magnitude_db(&x, 0, &mut col);
        assert_eq!(col.len(), m.len());
        assert!((col[k] - m[k]).abs() < 1e-3, "{} vs {}", col[k], m[k]);
        // Long input stays within the column cap.
        let long = vec![0.25f32; 4095 * 16];
        assert!(spectrogram_db(&long, 64, 16).len() <= MAX_SPECTROGRAM_COLUMNS);
        assert_eq!(magma_color(-120.0, -90.0, 0.0), [0, 0, 4]);
        assert_eq!(magma_color(0.0, -90.0, 0.0), [252, 253, 191]);
    }
}
