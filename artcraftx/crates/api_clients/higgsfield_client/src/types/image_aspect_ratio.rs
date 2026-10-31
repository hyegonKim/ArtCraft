use crate::types::string_enum::string_enum;

string_enum! {
  /// Output aspect ratio for image generation — the union of every ratio the
  /// web app offers across models (each model's request type narrows this to
  /// its own list). Also what job params parse back into.
  ImageAspectRatio {
    /// Let the model pick (the web app offers this for reference-image
    /// workflows). NB: wire value inferred from the UI label; not yet seen
    /// in a captured request.
    Auto => "auto",
    Square1x1 => "1:1",
    Portrait2x3 => "2:3",
    Landscape3x2 => "3:2",
    Portrait3x4 => "3:4",
    Landscape4x3 => "4:3",
    Portrait4x5 => "4:5",
    Landscape5x4 => "5:4",
    Portrait9x16 => "9:16",
    Landscape16x9 => "16:9",
    Landscape21x9 => "21:9",
  }
}

impl ImageAspectRatio {
  /// Width / height as a ratio, e.g. `16:9` → `1.777…`. `Auto` has none;
  /// `Other` values are parsed as `W:H` when possible.
  pub fn ratio(&self) -> Option<f64> {
    let (width, height) = self.as_str().split_once(':')?;
    let width: f64 = width.parse().ok()?;
    let height: f64 = height.parse().ok()?;
    if height <= 0.0 || width <= 0.0 {
      return None;
    }
    Some(width / height)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn wire_round_trip() {
    for variant in ImageAspectRatio::known_variants() {
      assert_eq!(&ImageAspectRatio::from_str_lossy(variant.as_str()), variant);
    }
  }

  #[test]
  fn serde_uses_wire_string() {
    assert_eq!(serde_json::to_string(&ImageAspectRatio::Portrait3x4).unwrap(), "\"3:4\"");
    let parsed: ImageAspectRatio = serde_json::from_str("\"16:9\"").unwrap();
    assert_eq!(parsed, ImageAspectRatio::Landscape16x9);
  }

  #[test]
  fn unknown_parses_as_other() {
    let parsed: ImageAspectRatio = serde_json::from_str("\"7:5\"").unwrap();
    assert_eq!(parsed, ImageAspectRatio::Other("7:5".to_string()));
    assert_eq!(parsed.ratio(), Some(1.4));
  }

  #[test]
  fn ratio() {
    assert_eq!(ImageAspectRatio::Square1x1.ratio(), Some(1.0));
    assert!((ImageAspectRatio::Landscape16x9.ratio().unwrap() - 16.0 / 9.0).abs() < 1e-9);
    assert_eq!(ImageAspectRatio::Other("nope".to_string()).ratio(), None);
    assert_eq!(ImageAspectRatio::Auto.ratio(), None);
  }
}
