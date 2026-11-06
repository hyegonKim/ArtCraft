//! RGB Levels adjustment for the local image editor. Parameters are applied to the composite,
//! leaving source layer pixels untouched. This is an 8-bit RGB operation, not color management.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
  pub input_black: u8,
  pub input_white: u8,
  pub gamma: f32,
  pub output_black: u8,
  pub output_white: u8,
}

impl Default for Levels {
  fn default() -> Self {
    Self { input_black: 0, input_white: 255, gamma: 1.0, output_black: 0, output_white: 255 }
  }
}

impl Levels {
  /// Remaps the input black/white points, applies midtone gamma, then maps to the output range.
  /// Alpha and any incomplete trailing pixel are preserved. An inverted output range is allowed.
  pub fn apply(self, rgba: &mut [u8]) {
    if self == Self::default() {
      return;
    }
    // Keep even programmatically supplied parameters finite and the input interval nonempty.
    let black = self.input_black.min(254) as f32;
    let white = (self.input_white as f32).max(black + 1.0);
    let gamma = if self.gamma.is_finite() { self.gamma.clamp(0.1, 9.99) } else { 1.0 };
    let output_black = self.output_black as f32;
    let output_span = self.output_white as f32 - output_black;
    let mut table = [0u8; 256];
    for (value, mapped) in table.iter_mut().enumerate() {
      let normalized = ((value as f32 - black) / (white - black)).clamp(0.0, 1.0);
      *mapped = (output_black + normalized.powf(gamma.recip()) * output_span).clamp(0.0, 255.0).round() as u8;
    }
    for pixel in rgba.chunks_exact_mut(4) {
      for channel in &mut pixel[..3] {
        *channel = table[*channel as usize];
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn identity_preserves_every_rgb_value_and_alpha() {
    let mut pixels: Vec<u8> = (0..=255).flat_map(|v| [v, 255 - v, v, v]).collect();
    let original = pixels.clone();
    Levels::default().apply(&mut pixels);
    assert_eq!(pixels, original);
  }

  #[test]
  fn input_points_clip_and_stretch_midtones() {
    let mut pixels = [32, 64, 128, 37, 192, 224, 96, 199];
    Levels { input_black: 64, input_white: 192, ..Levels::default() }.apply(&mut pixels);
    assert_eq!(pixels, [0, 0, 128, 37, 255, 255, 64, 199]);
  }

  #[test]
  fn gamma_brightens_or_darkens_midtones_without_moving_endpoints() {
    let mut bright = [0, 64, 255, 128];
    let mut dark = bright;
    Levels { gamma: 2.0, ..Levels::default() }.apply(&mut bright);
    Levels { gamma: 0.5, ..Levels::default() }.apply(&mut dark);
    assert_eq!(bright, [0, 128, 255, 128]);
    assert_eq!(dark, [0, 16, 255, 128]);
  }

  #[test]
  fn output_points_can_compress_or_invert_the_range() {
    let mut pixels = [0, 128, 255, 17];
    Levels { output_black: 20, output_white: 220, ..Levels::default() }.apply(&mut pixels);
    assert_eq!(pixels, [20, 120, 220, 17]);
    let mut inverted = [0, 128, 255, 17];
    Levels { output_black: 255, output_white: 0, ..Levels::default() }.apply(&mut inverted);
    assert_eq!(inverted, [255, 127, 0, 17]);
  }

  #[test]
  fn invalid_inputs_and_nonfinite_gamma_are_safely_normalized() {
    let mut pixels = [0, 254, 255, 77];
    Levels { input_black: 255, input_white: 0, gamma: f32::NAN, ..Levels::default() }.apply(&mut pixels);
    assert_eq!(pixels, [0, 0, 255, 77]);
    Levels { gamma: f32::INFINITY, ..Levels::default() }.apply(&mut pixels);
    assert_eq!(pixels, [0, 0, 255, 77]);
  }

  #[test]
  fn empty_and_incomplete_pixels_are_safe() {
    let levels = Levels { gamma: 2.0, ..Levels::default() };
    levels.apply(&mut []);
    let mut pixels = [64, 64, 64, 42, 12, 34];
    levels.apply(&mut pixels);
    assert_eq!(pixels, [128, 128, 128, 42, 12, 34]);
  }
}
