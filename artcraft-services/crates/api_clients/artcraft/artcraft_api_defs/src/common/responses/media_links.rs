use serde::Deserialize;
use serde::Serialize;
use url::Url;
use utoipa::ToSchema;

/// Links to media file locations (bucket, CDN, etc.)
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Eq, PartialEq)]
pub struct MediaLinks {
  /// Primary link to the asset via the CDN.
  pub cdn_url: Url,

  /// Template to construct thumbnail URLs.
  /// Replace the string `{WIDTH}` with the desired width.
  /// Only relevant for image media files. (Video media files instead have
  /// video previews, which, in turn, have their own thumbnail templates.)
  pub maybe_thumbnail_template: Option<String>,

  /// Video preview images (still, animated webp or gif, and a Catalina gif
  /// fallback) for mp4 video files.
  /// These are only set for video media files.
  /// These are not set for "cover images" that are gif or webms, but rather video files.
  pub maybe_video_previews: Option<VideoPreviews>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Eq, PartialEq)]
pub struct VideoPreviews {
  /// A static single frame preview image of the video.
  pub still: Url,
  
  /// An animated preview of the video: an animated webp for videos with
  /// thumbnail version 2 (2026-09+), otherwise a gif. Clients that can't
  /// render animated webp should use `maybe_catalina_fallback_animated_gif`.
  pub animated: Url,
  
  /// A template used to construct the still thumbnail URL.
  /// Replace the string `{WIDTH}` with the desired width.
  pub still_thumbnail_template: String,
  
  /// A template used to construct the animated thumbnail URL.
  /// Replace the string `{WIDTH}` with the desired width.
  pub animated_thumbnail_template: String,

  /// An animated gif preview of the video, for clients that can't render the
  /// webp in `animated`: ArtCraft desktop on macOS 10.15 (Catalina) runs on a
  /// WKWebView without animated webp support. Since thumbnail version 2
  /// (2026-09) the thumbnail job writes this gif beside the webp; for older
  /// videos it's the same file as `animated`. See `video_thumbnail_suffixes`
  /// in `bucket_paths` for the version history.
  ///
  /// Always set by the server. Optional (and defaulted) only so newer clients
  /// can still read responses from older servers.
  #[serde(default)]
  pub maybe_catalina_fallback_animated_gif: Option<Url>,

  /// A template used to construct the Catalina fallback gif thumbnail URL.
  /// Replace the string `{WIDTH}` with the desired width.
  #[serde(default)]
  pub maybe_catalina_fallback_animated_gif_thumbnail_template: Option<String>,
}
