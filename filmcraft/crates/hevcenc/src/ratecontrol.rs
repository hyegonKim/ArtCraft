//! Frame-level rate control: a constant QP, or a one-pass average bitrate that steers the QP of
//! each frame from the bits the previous frames took (keyframes a few steps finer).

/// How the bitrate is controlled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RateControl {
    /// Every frame at this QP (0..=51).
    ConstantQp(u8),
    /// An average bitrate in kilobits per second, with a ceiling the QP rises to respect.
    Average { kbps: u32, max_kbps: u32 },
}

pub struct RateController {
    mode: RateControl,
    /// Target bits per frame.
    target: f64,
    /// Ceiling bits per frame (one second at max_kbps spread over the frames).
    ceiling: f64,
    /// The P-frame QP as a real number.
    qp: f64,
    /// Bits spent minus bits budgeted so far.
    debt: f64,
    frames: u64,
}

impl RateController {
    pub fn new(mode: RateControl, width: u32, height: u32, fps: f64) -> RateController {
        let fps = if fps.is_finite() && fps > 0.0 { fps } else { 30.0 };
        let (target, ceiling, qp) = match mode {
            RateControl::ConstantQp(q) => (0.0, 0.0, f64::from(q.min(51))),
            RateControl::Average { kbps, max_kbps } => {
                let target = f64::from(kbps.max(1)) * 1000.0 / fps;
                let ceiling = f64::from(max_kbps.max(kbps).max(1)) * 1000.0 / fps;
                // starting point from the bits per pixel: about 0.1 bpp at QP 30, 6 QP per halving
                let bpp = target / (f64::from(width.max(1)) * f64::from(height.max(1)));
                let qp = 30.0 - 6.0 * (bpp / 0.1).log2();
                (target, ceiling, qp.clamp(10.0, 46.0))
            }
        };
        RateController { mode, target, ceiling, qp, debt: 0.0, frames: 0 }
    }

    /// The QP of the next frame.
    pub fn frame_qp(&self, key: bool) -> i32 {
        match self.mode {
            RateControl::ConstantQp(q) => i32::from(q.min(51)),
            RateControl::Average { .. } => {
                let q = if key { self.qp - 3.0 } else { self.qp };
                (q.round() as i32).clamp(1, 51)
            }
        }
    }

    /// Account for a coded frame of `bits` bits.
    pub fn update(&mut self, bits: f64, key: bool) {
        if let RateControl::Average { .. } = self.mode {
            self.frames += 1;
            // keyframes are budgeted at four times a P frame
            let budget = if key { self.target * 4.0 } else { self.target };
            self.debt += bits - self.target;
            let ratio = (bits / budget.max(1.0)).max(1e-3);
            // move towards the QP that would have hit the budget, plus a correction for the debt
            // accumulated over about two seconds of frames
            let step = 6.0 * ratio.log2();
            let debt_step = 6.0 * ((self.debt / (self.target * 50.0)) + 1.0).max(0.25).log2();
            let mut delta = (0.5 * step + 0.5 * debt_step).clamp(-3.0, 3.0);
            if bits > self.ceiling * if key { 4.0 } else { 1.0 } {
                delta = delta.max(1.0);
            }
            self.qp = (self.qp + delta).clamp(1.0, 51.0);
        }
    }
}
