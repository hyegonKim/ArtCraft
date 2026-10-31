use crate::prompt_flags::generation_mode::GenerationMode;

/// Long-argument flags Grok reads out of the *prompt text* itself. The web app
/// mutates a prompt into e.g. `"woman on beach --mode=extremely-spicy-or-crazy"`
/// before sending it, for both image and video generation.
///
/// Callers set the flags they want and call [`apply_to`](Self::apply_to) to get
/// the final prompt string. This keeps prompt construction in one place and
/// unit-testable, separate from the request wire structs.
///
/// `Default` is "no flags" — [`apply_to`](Self::apply_to) then returns the
/// prompt unchanged (trimmed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptFlags {
  /// `--mode=<value>` — the generation style. `None` appends no `--mode`.
  pub mode: Option<GenerationMode>,
  // Additional `--flag` long args go here as they are confirmed from captures /
  // reference clients; `long_args` and `apply_to` pick them up automatically.
}

impl PromptFlags {
  /// Flags carrying just a `--mode`.
  pub fn with_mode(mode: GenerationMode) -> Self {
    Self { mode: Some(mode) }
  }

  /// Whether no flags are set (so [`apply_to`](Self::apply_to) is a no-op).
  pub fn is_empty(&self) -> bool {
    self.long_args().is_empty()
  }

  /// Append the set flags to `prompt`, yielding the final text Grok expects,
  /// e.g. `apply_to("woman on beach")` → `"woman on beach --mode=..."`.
  ///
  /// The prompt is trimmed; flags are space-separated after it. An empty prompt
  /// yields just the flags (no leading space), matching the web app's bare
  /// image-to-video frame `"--mode=normal"`.
  pub fn apply_to(&self, prompt: &str) -> String {
    append_long_args(prompt, &self.long_args())
  }

  /// The ordered `(name, value)` long args this set produces. Order is fixed so
  /// output is deterministic (and testable).
  fn long_args(&self) -> Vec<(&'static str, LongArgValue)> {
    let mut args: Vec<(&'static str, LongArgValue)> = Vec::new();
    if let Some(mode) = self.mode {
      args.push(("mode", LongArgValue::Equals(mode.as_flag_value())));
    }
    args
  }
}

/// A `--flag` may be bare (`--foo`) or take a value (`--foo=bar`).
enum LongArgValue {
  #[allow(dead_code)] // used once a bare-flag is added
  Bare,
  Equals(&'static str),
}

/// Append `--name` / `--name=value` long args to `prompt`. Shared mechanical
/// core so the string-building rules live (and are tested) in one place.
fn append_long_args(prompt: &str, args: &[(&str, LongArgValue)]) -> String {
  let mut out = prompt.trim().to_string();
  for (name, value) in args {
    if !out.is_empty() {
      out.push(' ');
    }
    out.push_str("--");
    out.push_str(name);
    if let LongArgValue::Equals(value) = value {
      out.push('=');
      out.push_str(value);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn no_flags_returns_trimmed_prompt() {
    assert_eq!(PromptFlags::default().apply_to("  a red kite  "), "a red kite");
    assert!(PromptFlags::default().is_empty());
  }

  #[test]
  fn mode_is_appended_as_long_arg() {
    assert_eq!(
      PromptFlags::with_mode(GenerationMode::Spicy).apply_to("woman on beach"),
      "woman on beach --mode=extremely-spicy-or-crazy",
    );
    assert_eq!(
      PromptFlags::with_mode(GenerationMode::Custom).apply_to("An asteroid hits the city."),
      "An asteroid hits the city. --mode=custom",
    );
  }

  #[test]
  fn empty_prompt_yields_bare_flags() {
    // Matches the web app's bare image-to-video frame.
    assert_eq!(PromptFlags::with_mode(GenerationMode::Normal).apply_to(""), "--mode=normal");
    assert_eq!(PromptFlags::with_mode(GenerationMode::Normal).apply_to("   "), "--mode=normal");
  }
}
