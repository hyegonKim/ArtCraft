/// The `--mode` selector Grok appends to an image/video prompt to choose the
/// generation style. Wire values confirmed from real Imagine captures
/// (2026-08-24): image gen sends e.g. `"woman on beach --mode=extremely-spicy-or-crazy"`,
/// and video gen sends `"<prompt> --mode=custom"` / `"--mode=normal"`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GenerationMode {
  /// `--mode=normal` — the default, safe style.
  Normal,

  /// `--mode=extremely-crazy` — the "fun" style.
  Fun,

  /// `--mode=extremely-spicy-or-crazy` — the "spicy" / NSFW style.
  Spicy,

  /// `--mode=custom` — honors the caller's text prompt (the other modes lean on
  /// their own styling). Video text-to-video uses this.
  Custom,
}

impl GenerationMode {
  /// The value used after `--mode=` (and, for image-to-video, in the structured
  /// `mode` field).
  pub fn as_flag_value(self) -> &'static str {
    match self {
      Self::Normal => "normal",
      Self::Fun => "extremely-crazy",
      Self::Spicy => "extremely-spicy-or-crazy",
      Self::Custom => "custom",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn flag_values_match_captures() {
    assert_eq!(GenerationMode::Normal.as_flag_value(), "normal");
    assert_eq!(GenerationMode::Fun.as_flag_value(), "extremely-crazy");
    assert_eq!(GenerationMode::Spicy.as_flag_value(), "extremely-spicy-or-crazy");
    assert_eq!(GenerationMode::Custom.as_flag_value(), "custom");
  }
}
