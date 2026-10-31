//! Which `ffmpeg` binary to use for animated webp previews.
//!
//! Animated webp needs ffmpeg's `libwebp_anim` encoder. Ubuntu's `ffmpeg`
//! package (production) has it. Homebrew's slimmed `ffmpeg` formula doesn't
//! (as of 2026-09); its full build is the keg-only `ffmpeg-full` formula,
//! which isn't on the PATH. So on macOS development machines we fall back to
//! `ffmpeg-full` when the `ffmpeg` on the PATH can't encode webp.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::anyhow;
use log::{info, warn};

use errors::AnyhowResult;

/// The `ffmpeg` on the PATH.
const PATH_FFMPEG: &str = "ffmpeg";

/// Homebrew's keg-only `ffmpeg-full` (Apple Silicon, then Intel prefixes).
const HOMEBREW_FFMPEG_FULL_CANDIDATES: [&str; 2] = [
  "/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg",
  "/usr/local/opt/ffmpeg-full/bin/ffmpeg",
];

/// The process-wide decision. Resolved once (the first caller probes the
/// binaries while concurrent callers wait), then shared by every thread.
/// Failures are cached too: a missing encoder won't appear mid-process.
static WEBP_FFMPEG: OnceLock<Result<PathBuf, String>> = OnceLock::new();

/// Resolve (once per process, then cached) the `ffmpeg` binary that can
/// encode animated webp.
///
/// Uses the `ffmpeg` on the PATH if it has `libwebp_anim`. Otherwise, only when
/// `is_development` and on macOS, falls back to Homebrew's `ffmpeg-full` and
/// logs a warning. Call this at startup with the server environment; the first
/// call decides for the whole process, and later calls return the cached
/// decision (whatever `is_development` they pass).
pub fn resolve_webp_ffmpeg(is_development: bool) -> AnyhowResult<&'static Path> {
  let decision = WEBP_FFMPEG.get_or_init(|| {
    let allow_macos_fallback = is_development && cfg!(target_os = "macos");
    let decision = choose_webp_ffmpeg(
      has_webp_encoder(Path::new(PATH_FFMPEG)),
      allow_macos_fallback,
      find_homebrew_ffmpeg_full,
    );
    match &decision {
      Ok(WebpFfmpegChoice::Path) =>
        info!("Using the ffmpeg on the PATH for webp previews."),
      Ok(WebpFfmpegChoice::HomebrewFallback(binary)) =>
        warn!("The ffmpeg on the PATH has no libwebp_anim encoder; falling back to {} for webp previews \
               (macOS development only).", binary.display()),
      Err(err) =>
        warn!("No ffmpeg can encode webp previews: {}", err),
    }
    decision.map(WebpFfmpegChoice::into_binary)
  });

  decision.as_deref().map_err(|err| anyhow!("{}", err))
}

/// The binary resolved by [`resolve_webp_ffmpeg`], or production behavior
/// (the PATH only) if nothing resolved it yet.
pub(crate) fn webp_ffmpeg() -> AnyhowResult<&'static Path> {
  resolve_webp_ffmpeg(false)
}

#[derive(Debug, PartialEq, Eq)]
enum WebpFfmpegChoice {
  Path,
  HomebrewFallback(PathBuf),
}

impl WebpFfmpegChoice {
  fn into_binary(self) -> PathBuf {
    match self {
      WebpFfmpegChoice::Path => PathBuf::from(PATH_FFMPEG),
      WebpFfmpegChoice::HomebrewFallback(binary) => binary,
    }
  }
}

/// The decision, separated from probing binaries so it can be tested.
fn choose_webp_ffmpeg(
  path_ffmpeg_has_webp: bool,
  allow_macos_fallback: bool,
  find_fallback: impl FnOnce() -> Option<PathBuf>,
) -> Result<WebpFfmpegChoice, String> {
  if path_ffmpeg_has_webp {
    return Ok(WebpFfmpegChoice::Path);
  }
  if !allow_macos_fallback {
    return Err("the ffmpeg on the PATH has no libwebp_anim encoder, which animated webp thumbnails need".to_string());
  }
  find_fallback()
      .map(WebpFfmpegChoice::HomebrewFallback)
      .ok_or_else(|| "neither the ffmpeg on the PATH nor Homebrew's ffmpeg-full has a libwebp_anim encoder. \
          Run `brew install ffmpeg-full`.".to_string())
}

/// The first installed Homebrew `ffmpeg-full` that has the webp encoder.
fn find_homebrew_ffmpeg_full() -> Option<PathBuf> {
  HOMEBREW_FFMPEG_FULL_CANDIDATES.iter()
      .map(PathBuf::from)
      .find(|binary| binary.exists() && has_webp_encoder(binary))
}

/// Whether `binary -encoders` lists `libwebp_anim`. False if it can't run.
fn has_webp_encoder(binary: &Path) -> bool {
  Command::new(binary)
      .args(["-hide_banner", "-encoders"])
      .output()
      .map(|output| String::from_utf8_lossy(&output.stdout)
          .split_whitespace()
          .any(|word| word == "libwebp_anim"))
      .unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;

  const FALLBACK: &str = "/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg";

  #[test]
  fn uses_path_ffmpeg_when_it_has_webp() {
    let choice = choose_webp_ffmpeg(true, true, || panic!("shouldn't look for a fallback"));
    assert_eq!(choice, Ok(WebpFfmpegChoice::Path));
  }

  #[test]
  fn falls_back_to_ffmpeg_full_when_allowed() {
    let choice = choose_webp_ffmpeg(false, true, || Some(PathBuf::from(FALLBACK)));
    assert_eq!(choice, Ok(WebpFfmpegChoice::HomebrewFallback(PathBuf::from(FALLBACK))));
  }

  #[test]
  fn never_falls_back_outside_macos_development() {
    let choice = choose_webp_ffmpeg(false, false, || panic!("shouldn't look for a fallback"));
    assert!(choice.is_err());
  }

  #[test]
  fn errors_when_the_fallback_is_missing() {
    let choice = choose_webp_ffmpeg(false, true, || None);
    let err = choice.unwrap_err();
    assert!(err.contains("brew install ffmpeg-full"), "{}", err);
  }

  #[test]
  fn choice_binaries() {
    assert_eq!(WebpFfmpegChoice::Path.into_binary(), PathBuf::from("ffmpeg"));
    assert_eq!(WebpFfmpegChoice::HomebrewFallback(PathBuf::from(FALLBACK)).into_binary(), PathBuf::from(FALLBACK));
  }
}
