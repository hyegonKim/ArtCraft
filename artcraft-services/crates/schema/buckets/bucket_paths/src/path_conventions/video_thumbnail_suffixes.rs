//! Shared config for video thumbnails (the still and animated previews stored
//! beside each video file in the public bucket).
//!
//! # Thumbnail versions (`media_files.maybe_thumbnail_version`)
//!
//! | Version | Since                    | Files beside `<video>.mp4`                                 |
//! |---------|--------------------------|------------------------------------------------------------|
//! | `NULL`  | 2024-02-28               | `-thumb.jpg`, `-thumb.gif` (legacy, or not yet generated)  |
//! | `1`     | 2026-03-18               | `-thumb.jpg`, `-thumb.gif`                                 |
//! | `2`     | 2026-09 (see below)      | `-thumb.jpg`, `-thumb.webp`, `-thumb.gif`                  |
//!
//! * **`NULL`**: No version recorded. Clients have linked `-thumb.gif` and
//!   `-thumb.jpg` since 2024-02-28 ("video card thumbnails", e7a427289f), so
//!   older rows may have GIF/JPG files from earlier pipelines, or none at all.
//!   A `NULL` row may also just be waiting for the thumbnail job. Serving uses
//!   the `created_at` cutover in [`animated_preview_format`] to pick GIF or WebP.
//! * **`1`**: Written by `video-thumbnail-job` since it launched on 2026-03-18
//!   (#1157). A first-frame JPG and an animated GIF: the first 5 seconds at
//!   10 fps, fit within 360x360.
//! * **`2`**: Same JPG, plus an animated WebP (quality 60) of the same frames,
//!   which is typically 6-50x smaller than the GIF. The job still writes the
//!   GIF too ("dual writes"); see "macOS Catalina" below. Deployed in late
//!   September 2026. TODO(bt): Replace "2026-09" above with the exact deploy date.
//!
//! # macOS Catalina and GIF dual writes
//!
//! Animated WebP needs WebKit on macOS 11 (Big Sur) or newer. ArtCraft desktop
//! (Tauri) uses the system WKWebView, so on macOS 10.15 (Catalina) a WebP in
//! an `<img>` doesn't render. Version 2 therefore keeps writing `-thumb.gif`
//! beside the WebP, and the API exposes it as the Catalina fallback so the
//! desktop app can fall back to it. We may keep the GIF dual writes forever;
//! drop them only once no supported client needs them.
//!
//! # Changing this scheme
//!
//! These suffixes are how files are physically named in the public bucket and
//! are shared between services. Never change or reuse a suffix or a version
//! number. Add a new version instead, and update [`animated_preview_format`].

/// NB: DO NOT CHANGE WITHOUT MIGRATION - THIS IS SHARED BETWEEN SERVICES
/// Video media files have conventional gif thumbnails with this suffix.
/// This is how files are physically named in the public bucket.
pub const VIDEO_ANIMATED_GIF_THUMBNAIL_SUFFIX: &str = "-thumb.gif";

/// NB: DO NOT CHANGE WITHOUT MIGRATION - THIS IS SHARED BETWEEN SERVICES
/// Video media files with thumbnail version 2+ have animated webp thumbnails
/// with this suffix. This is how files are physically named in the public bucket.
pub const VIDEO_ANIMATED_WEBP_THUMBNAIL_SUFFIX: &str = "-thumb.webp";

/// NB: DO NOT CHANGE WITHOUT MIGRATION - THIS IS SHARED BETWEEN SERVICES
/// Video media files have conventional static jpg thumbnails with this suffix.
/// This is how files are physically named in the public bucket.
pub const VIDEO_STATIC_JPG_THUMBNAIL_SUFFIX: &str = "-thumb.jpg";

/// Version 1 (2026-03-18): first-frame jpg + animated gif.
pub const VIDEO_THUMBNAIL_VERSION_GIF: u8 = 1;

/// Version 2 (2026-09): first-frame jpg + animated webp, plus the animated gif
/// as a dual write for macOS Catalina.
pub const VIDEO_THUMBNAIL_VERSION_WEBP: u8 = 2;

/// The version `video-thumbnail-job` writes for newly processed videos.
pub const CURRENT_VIDEO_THUMBNAIL_VERSION: u8 = VIDEO_THUMBNAIL_VERSION_WEBP;

/// Videos with a `NULL` thumbnail version created at or after this instant
/// (2026-10-10T00:00:00Z) are served WebP previews; older `NULL` rows get GIF.
///
/// TODO(bt): Remove this cutover; it shouldn't live forever. It exists
/// because `NULL` means two things today: "legacy row that only ever had a
/// GIF" and "new row the thumbnail job hasn't reached yet (it will write a
/// WebP)". To remove it:
///   1. Wait until the thumbnail job's lookback window has passed since
///      2026-10-10, so every row created after the cutover has been processed.
///   2. Backfill legacy rows older than the lookback window, eg.
///      `UPDATE media_files SET maybe_thumbnail_version = 1
///       WHERE maybe_thumbnail_version IS NULL AND media_class = 'video'
///       AND created_at < '2026-10-10'`.
///      (Rows inside the lookback window must be left `NULL`, or the job
///      will skip them.)
///   3. `NULL` then only means "not generated yet", so serve the current
///      format for it: delete this constant and the `created_at` argument to
///      [`animated_preview_format`], and stop selecting `created_at` for it.
pub const NULL_THUMBNAIL_VERSION_WEBP_CUTOVER_UNIX_SECONDS: i64 = 1_791_590_400;

/// The format of a video's animated preview file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimatedPreviewFormat {
  Gif,
  Webp,
}

impl AnimatedPreviewFormat {
  /// The bucket file suffix, eg. `-thumb.webp`.
  pub const fn suffix(self) -> &'static str {
    match self {
      Self::Gif => VIDEO_ANIMATED_GIF_THUMBNAIL_SUFFIX,
      Self::Webp => VIDEO_ANIMATED_WEBP_THUMBNAIL_SUFFIX,
    }
  }
}

/// Which animated preview to serve for a video, from its
/// `media_files.maybe_thumbnail_version` and `created_at` (as Unix seconds).
/// See the module docs for the version history.
pub fn animated_preview_format(
  maybe_thumbnail_version: Option<u8>,
  created_at_unix_seconds: i64,
) -> AnimatedPreviewFormat {
  match maybe_thumbnail_version {
    Some(version) if version >= VIDEO_THUMBNAIL_VERSION_WEBP => AnimatedPreviewFormat::Webp,
    Some(_) => AnimatedPreviewFormat::Gif,
    None if created_at_unix_seconds >= NULL_THUMBNAIL_VERSION_WEBP_CUTOVER_UNIX_SECONDS => AnimatedPreviewFormat::Webp,
    None => AnimatedPreviewFormat::Gif,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // 2026-10-09T23:59:59Z and 2026-10-10T00:00:00Z
  const JUST_BEFORE_CUTOVER: i64 = 1_791_590_399;
  const AT_CUTOVER: i64 = 1_791_590_400;
  // 2024-05-01T00:00:00Z
  const LEGACY_ERA: i64 = 1_714_521_600;

  #[test]
  fn suffixes_are_stable() {
    assert_eq!(VIDEO_ANIMATED_GIF_THUMBNAIL_SUFFIX, "-thumb.gif");
    assert_eq!(VIDEO_ANIMATED_WEBP_THUMBNAIL_SUFFIX, "-thumb.webp");
    assert_eq!(VIDEO_STATIC_JPG_THUMBNAIL_SUFFIX, "-thumb.jpg");
    assert_eq!(AnimatedPreviewFormat::Gif.suffix(), "-thumb.gif");
    assert_eq!(AnimatedPreviewFormat::Webp.suffix(), "-thumb.webp");
  }

  #[test]
  fn versions_are_stable() {
    assert_eq!(VIDEO_THUMBNAIL_VERSION_GIF, 1);
    assert_eq!(VIDEO_THUMBNAIL_VERSION_WEBP, 2);
    assert_eq!(CURRENT_VIDEO_THUMBNAIL_VERSION, 2);
  }

  mod animated_preview_format_tests {
    use super::*;

    #[test]
    fn version_1_is_gif_regardless_of_date() {
      assert_eq!(animated_preview_format(Some(1), LEGACY_ERA), AnimatedPreviewFormat::Gif);
      assert_eq!(animated_preview_format(Some(1), AT_CUTOVER), AnimatedPreviewFormat::Gif);
    }

    #[test]
    fn version_2_is_webp_regardless_of_date() {
      assert_eq!(animated_preview_format(Some(2), LEGACY_ERA), AnimatedPreviewFormat::Webp);
      assert_eq!(animated_preview_format(Some(2), AT_CUTOVER), AnimatedPreviewFormat::Webp);
    }

    #[test]
    fn null_before_cutover_is_gif() {
      assert_eq!(animated_preview_format(None, LEGACY_ERA), AnimatedPreviewFormat::Gif);
      assert_eq!(animated_preview_format(None, JUST_BEFORE_CUTOVER), AnimatedPreviewFormat::Gif);
    }

    #[test]
    fn null_at_or_after_cutover_is_webp() {
      assert_eq!(animated_preview_format(None, AT_CUTOVER), AnimatedPreviewFormat::Webp);
      assert_eq!(animated_preview_format(None, AT_CUTOVER + 86_400), AnimatedPreviewFormat::Webp);
    }

    #[test]
    fn cutover_is_2026_10_10_utc() {
      // 20_736 days from 1970-01-01 to 2026-10-10.
      assert_eq!(NULL_THUMBNAIL_VERSION_WEBP_CUTOVER_UNIX_SECONDS, 20_736 * 86_400);
    }
  }
}
