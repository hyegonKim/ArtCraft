use crate::http_server::common_responses::media::cdn_link::{get_cdn_host, new_cdn_url};
use crate::http_server::common_responses::media::media_domain::MediaDomain;
use artcraft_api_defs::common::responses::media_links::{MediaLinks, VideoPreviews};
use bucket_paths::legacy::typified_paths::public::media_files::bucket_file_path::MediaFileBucketPath;
use bucket_paths::path_conventions::video_thumbnail_suffixes::{animated_preview_format, AnimatedPreviewFormat, VIDEO_STATIC_JPG_THUMBNAIL_SUFFIX};
use chrono::{DateTime, Utc};
use server_environment::ServerEnvironment;
use url::Url;

// TODO(bt,2024-09-05): Worth reducing the quality at all?
const QUALITY : u8 = 95;


/// Links to media file locations (bucket, CDN, etc.)
/// 
/// This migrates the constructors to the `artcraft_api_defs` crate
pub struct MediaLinksBuilder {}

/// The `media_files` columns that decide which video preview files exist.
/// Required for every media file (not just videos) so no read site can build
/// video previews without them. See `video_thumbnail_suffixes` in
/// `bucket_paths` for the thumbnail version history.
#[derive(Copy, Clone, Debug)]
pub struct VideoThumbnailInfo {
  /// `media_files.maybe_thumbnail_version`
  pub maybe_thumbnail_version: Option<u8>,
  /// `media_files.created_at` (decides `NULL` versions around the cutover)
  pub created_at: DateTime<Utc>,
}

impl VideoThumbnailInfo {
  pub fn new(maybe_thumbnail_version: Option<u8>, created_at: DateTime<Utc>) -> Self {
    Self { maybe_thumbnail_version, created_at }
  }
}

impl MediaLinksBuilder {

  pub fn from_media_path_and_env(
    domain: MediaDomain,
    server_environment: ServerEnvironment,
    bucket_path: &MediaFileBucketPath,
    thumbnail_info: VideoThumbnailInfo,
  ) -> MediaLinks {
    let rooted_path = bucket_path.get_full_object_path_str();
    Self::from_rooted_path_and_env(domain, server_environment, rooted_path, thumbnail_info)
  }

  pub fn from_rooted_path_and_env(
    domain: MediaDomain,
    server_environment: ServerEnvironment,
    rooted_path: &str,
    thumbnail_info: VideoThumbnailInfo,
  ) -> MediaLinks {
    let mut cdn_url = new_cdn_url(domain, server_environment);
    cdn_url.set_path(rooted_path);
    MediaLinks {
      cdn_url,
      maybe_thumbnail_template: thumbnail_template(domain, server_environment, rooted_path),
      maybe_video_previews: VideoPreviewsBuilder::from_rooted_path(domain, server_environment, rooted_path, thumbnail_info),
    }
  }
}

pub struct VideoPreviewsBuilder {}

impl VideoPreviewsBuilder {
  fn from_rooted_path(
    domain: MediaDomain,
    server_environment: ServerEnvironment,
    rooted_path: &str,
    thumbnail_info: VideoThumbnailInfo,
  ) -> Option<VideoPreviews> {
    if !rooted_path.ends_with(".mp4") {
      return None;
    }
    let animated = PreviewType::Animated(animated_preview_format(
      thumbnail_info.maybe_thumbnail_version,
      thumbnail_info.created_at.timestamp()));
    // Written for every thumbnail version (dual write since version 2).
    let catalina_fallback = PreviewType::Animated(AnimatedPreviewFormat::Gif);
    Some(VideoPreviews {
      still: video_preview(domain, server_environment, rooted_path, PreviewType::Still),
      animated: video_preview(domain, server_environment, rooted_path, animated),
      still_thumbnail_template: video_preview_thumbnail_template(domain, server_environment, rooted_path, PreviewType::Still),
      animated_thumbnail_template: video_preview_thumbnail_template(domain, server_environment, rooted_path, animated),
      maybe_catalina_fallback_animated_gif: Some(video_preview(domain, server_environment, rooted_path, catalina_fallback)),
      maybe_catalina_fallback_animated_gif_thumbnail_template: Some(video_preview_thumbnail_template(domain, server_environment, rooted_path, catalina_fallback)),
    })
  }
}

#[derive(Copy, Clone)]
enum PreviewType {
  /// First-frame jpg.
  Still,
  /// Animated gif or webp.
  Animated(AnimatedPreviewFormat),
}

impl PreviewType {
  fn suffix(self) -> &'static str {
    match self {
      PreviewType::Still => VIDEO_STATIC_JPG_THUMBNAIL_SUFFIX,
      PreviewType::Animated(format) => format.suffix(),
    }
  }
}

/// Returns a still or animated preview of the video.
fn video_preview(media_domain: MediaDomain, server_environment: ServerEnvironment, rooted_path: &str, preview_type: PreviewType) -> Url {
  let rooted_path = format!("{rooted_path}{}", preview_type.suffix());
  let mut url = new_cdn_url(media_domain, server_environment);
  url.set_path(&rooted_path);
  url
}

/// Returns a thumbnail template for image
fn thumbnail_template(media_domain: MediaDomain, server_environment: ServerEnvironment, rooted_path: &str) -> Option<String> {
  if !rooted_path.ends_with(".jpg")
      && !rooted_path.ends_with(".png")
      && !rooted_path.ends_with(".gif") {
    return None;
  }

  let host = get_cdn_host(media_domain, server_environment);

  // NB(bt,2025-02-01): Development doesn't currently support thumbnails, so serve the full image.
  match server_environment {
    ServerEnvironment::Development => Some(format!("{host}{rooted_path}")), // NB(bt,2025-02-01): No thumbnails in development.
    ServerEnvironment::Production => Some(format!("{host}/cdn-cgi/image/width={{WIDTH}},quality={QUALITY}{rooted_path}"))
  }
}

/// Returns a thumbnail template for video
///
/// TODO(bt,2026-09-28): Verify Cloudflare's `/cdn-cgi/image` resizing keeps
/// animated webp previews (thumbnail version 2) animated, at a few widths,
/// once real version 2 thumbnails exist in production. Cloudflare's docs
/// didn't confirm animated webp support when this was written. If it doesn't
/// work, serve webp previews without `/cdn-cgi/image` (they're already small).
fn video_preview_thumbnail_template(media_domain: MediaDomain, server_environment: ServerEnvironment, rooted_path: &str, preview_type: PreviewType) -> String {
  let host = get_cdn_host(media_domain, server_environment);
  let rooted_path = format!("{rooted_path}{}", preview_type.suffix());
  // NB: Development doesn't support cdn-cgi image resizing.
  match server_environment {
    ServerEnvironment::Development => format!("{host}{rooted_path}"),
    ServerEnvironment::Production => format!("{host}/cdn-cgi/image/width={{WIDTH}},quality={QUALITY}{rooted_path}"),
  }
}


#[cfg(test)]
mod tests {
  use crate::http_server::common_responses::media::media_domain::MediaDomain;
  use crate::http_server::common_responses::media::media_links_builder::{MediaLinksBuilder, VideoThumbnailInfo};
  use bucket_paths::legacy::typified_paths::public::media_files::bucket_file_path::MediaFileBucketPath;
  use chrono::{DateTime, TimeZone, Utc};
  use server_environment::ServerEnvironment;

  const DEV_CDN: &str = "https://pub-c8a4a5bdbdb048f286b77bdf9f786ff2.r2.dev";
  const PROD_CDN: &str = "https://cdn-2.fakeyou.com";

  mod fakeyou {
    use super::*;

    const DOMAIN : MediaDomain = MediaDomain::FakeYou;

    mod production {
      use super::*;

      const ENV : ServerEnvironment = ServerEnvironment::Production;

      #[test]
      fn wav_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.wav", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn glb_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.glb", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.glb"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn jpg_image() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.jpg", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.jpg"));
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.jpg")));
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn mp4_video() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.mp4", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.mp4"));
        assert_eq!(links.maybe_thumbnail_template, None);
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{PROD_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{PROD_CDN}/foo/bar.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.gif"));
      }

      #[test]
      fn mp4_video_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("t6cnyw4g3e8k7carkk2bvrt6nd3fycjv", Some("storyteller_"), Some(".mp4"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4"));
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
      }

      #[test]
      fn wav_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("94a27nmbd0bqmd10tg0pp3hz45zytf67", Some("fakeyou_"), Some(".wav"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/9/4/a/2/7/94a27nmbd0bqmd10tg0pp3hz45zytf67/fakeyou_94a27nmbd0bqmd10tg0pp3hz45zytf67.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn png_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("37mb3gh8fmj85y21thvbv08bzv24atjt", Some("upload_"), Some(".png"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/3/7/m/b/3/37mb3gh8fmj85y21thvbv08bzv24atjt/upload_37mb3gh8fmj85y21thvbv08bzv24atjt.png"));
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/3/7/m/b/3/37mb3gh8fmj85y21thvbv08bzv24atjt/upload_37mb3gh8fmj85y21thvbv08bzv24atjt.png")));
        assert_eq!(links.maybe_video_previews, None);
      }
    }

    mod development {
      use super::*;

      const ENV : ServerEnvironment = ServerEnvironment::Development;

      #[test]
      fn wav_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.wav", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn jpg_image() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.jpg", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.jpg"));
        // NB: Development doesn't support cdn-cgi, so serve the full image.
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{DEV_CDN}/foo/bar.jpg")));
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn mp4_video() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.mp4", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.mp4"));
        assert_eq!(links.maybe_thumbnail_template, None);
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{DEV_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{DEV_CDN}/foo/bar.mp4-thumb.gif"));
        // NB: Development doesn't support cdn-cgi, so templates are plain URLs.
        assert_eq!(video_previews.still_thumbnail_template, format!("{DEV_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{DEV_CDN}/foo/bar.mp4-thumb.gif"));
      }

      #[test]
      fn mp4_video_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("t6cnyw4g3e8k7carkk2bvrt6nd3fycjv", Some("storyteller_"), Some(".mp4"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4"));
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
      }
    }
  }

  mod storyteller {
    use super::*;

    const DOMAIN : MediaDomain = MediaDomain::Storyteller;

    mod production {
      use super::*;

      const ENV : ServerEnvironment = ServerEnvironment::Production;

      #[test]
      fn wav_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.wav", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn glb_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.glb", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.glb"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn jpg_image() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.jpg", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.jpg"));
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.jpg")));
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn mp4_video() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.mp4", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/foo/bar.mp4"));
        assert_eq!(links.maybe_thumbnail_template, None);
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{PROD_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{PROD_CDN}/foo/bar.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.gif"));
      }

      #[test]
      fn mp4_video_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("t6cnyw4g3e8k7carkk2bvrt6nd3fycjv", Some("storyteller_"), Some(".mp4"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4"));
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{PROD_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
      }

      #[test]
      fn wav_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("94a27nmbd0bqmd10tg0pp3hz45zytf67", Some("fakeyou_"), Some(".wav"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/9/4/a/2/7/94a27nmbd0bqmd10tg0pp3hz45zytf67/fakeyou_94a27nmbd0bqmd10tg0pp3hz45zytf67.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn png_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("37mb3gh8fmj85y21thvbv08bzv24atjt", Some("upload_"), Some(".png"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{PROD_CDN}/media/3/7/m/b/3/37mb3gh8fmj85y21thvbv08bzv24atjt/upload_37mb3gh8fmj85y21thvbv08bzv24atjt.png"));
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/media/3/7/m/b/3/37mb3gh8fmj85y21thvbv08bzv24atjt/upload_37mb3gh8fmj85y21thvbv08bzv24atjt.png")));
        assert_eq!(links.maybe_video_previews, None);
      }
    }

    mod development {
      use super::*;

      const ENV : ServerEnvironment = ServerEnvironment::Development;

      #[test]
      fn wav_file() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.wav", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.wav"));
        assert_eq!(links.maybe_thumbnail_template, None);
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn jpg_image() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.jpg", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.jpg"));
        assert_eq!(links.maybe_thumbnail_template, Some(format!("{DEV_CDN}/foo/bar.jpg")));
        assert_eq!(links.maybe_video_previews, None);
      }

      #[test]
      fn mp4_video() {
        let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, "/foo/bar.mp4", version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/foo/bar.mp4"));
        assert_eq!(links.maybe_thumbnail_template, None);
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{DEV_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{DEV_CDN}/foo/bar.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{DEV_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{DEV_CDN}/foo/bar.mp4-thumb.gif"));
      }

      #[test]
      fn mp4_video_media_path() {
        let media_path = MediaFileBucketPath::from_object_hash("t6cnyw4g3e8k7carkk2bvrt6nd3fycjv", Some("storyteller_"), Some(".mp4"));
        let links = MediaLinksBuilder::from_media_path_and_env(DOMAIN, ENV, &media_path, version_1());
        assert_eq!(links.cdn_url.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4"));
        let video_previews = links.maybe_video_previews.expect("should have previews");
        assert_eq!(video_previews.still.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated.as_str(), format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
        assert_eq!(video_previews.still_thumbnail_template, format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.jpg"));
        assert_eq!(video_previews.animated_thumbnail_template, format!("{DEV_CDN}/media/t/6/c/n/y/t6cnyw4g3e8k7carkk2bvrt6nd3fycjv/storyteller_t6cnyw4g3e8k7carkk2bvrt6nd3fycjv.mp4-thumb.gif"));
      }
    }
  }

  /// Picking the animated preview by thumbnail version. See
  /// `video_thumbnail_suffixes` in `bucket_paths` for the version history.
  mod thumbnail_versions {
    use super::*;

    const DOMAIN : MediaDomain = MediaDomain::FakeYou;
    const ENV : ServerEnvironment = ServerEnvironment::Production;
    const VIDEO : &str = "/foo/bar.mp4";

    #[test]
    fn version_1_serves_gif() {
      assert_animated_suffix(info(Some(1), after_cutover()), "-thumb.gif");
    }

    #[test]
    fn version_2_serves_webp() {
      assert_animated_suffix(info(Some(2), before_cutover()), "-thumb.webp");
    }

    #[test]
    fn null_before_cutover_serves_gif() {
      assert_animated_suffix(info(None, before_cutover()), "-thumb.gif");
    }

    #[test]
    fn null_at_cutover_serves_webp() {
      assert_animated_suffix(info(None, cutover()), "-thumb.webp");
    }

    #[test]
    fn still_is_jpg_for_every_version() {
      for thumbnail_info in [info(None, before_cutover()), info(None, cutover()), info(Some(1), cutover()), info(Some(2), cutover())] {
        let previews = previews(thumbnail_info);
        assert_eq!(previews.still.as_str(), format!("{PROD_CDN}/foo/bar.mp4-thumb.jpg"));
        assert_eq!(previews.still_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.jpg"));
      }
    }

    #[test]
    fn catalina_fallback_is_always_the_gif() {
      for thumbnail_info in [info(None, before_cutover()), info(None, cutover()), info(Some(1), cutover()), info(Some(2), cutover())] {
        let previews = previews(thumbnail_info);
        assert_eq!(
          previews.maybe_catalina_fallback_animated_gif.as_ref().map(|url| url.as_str()),
          Some(format!("{PROD_CDN}/foo/bar.mp4-thumb.gif").as_str()));
        assert_eq!(
          previews.maybe_catalina_fallback_animated_gif_thumbnail_template,
          Some(format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4-thumb.gif")));
      }
    }

    #[test]
    fn development_webp_is_served_directly() {
      let links = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ServerEnvironment::Development, VIDEO, info(Some(2), cutover()));
      let previews = links.maybe_video_previews.unwrap();
      assert_eq!(previews.animated.as_str(), format!("{DEV_CDN}/foo/bar.mp4-thumb.webp"));
      assert_eq!(previews.animated_thumbnail_template, format!("{DEV_CDN}/foo/bar.mp4-thumb.webp"));
      assert_eq!(previews.maybe_catalina_fallback_animated_gif_thumbnail_template, Some(format!("{DEV_CDN}/foo/bar.mp4-thumb.gif")));
    }

    #[test]
    fn non_videos_ignore_the_version() {
      for path in ["/foo/bar.png", "/foo/bar.gif", "/foo/bar.wav"] {
        let version_1 = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, path, info(Some(1), cutover()));
        let version_2 = MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, path, info(Some(2), cutover()));
        assert_eq!(version_1, version_2);
        assert_eq!(version_2.maybe_video_previews, None);
      }
    }

    fn assert_animated_suffix(thumbnail_info: VideoThumbnailInfo, suffix: &str) {
      let previews = previews(thumbnail_info);
      assert_eq!(previews.animated.as_str(), format!("{PROD_CDN}/foo/bar.mp4{suffix}"));
      assert_eq!(previews.animated_thumbnail_template, format!("{PROD_CDN}/cdn-cgi/image/width={{WIDTH}},quality=95/foo/bar.mp4{suffix}"));
    }

    fn previews(thumbnail_info: VideoThumbnailInfo) -> artcraft_api_defs::common::responses::media_links::VideoPreviews {
      MediaLinksBuilder::from_rooted_path_and_env(DOMAIN, ENV, VIDEO, thumbnail_info)
          .maybe_video_previews
          .expect("mp4 should have previews")
    }

    fn cutover() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 10, 10, 0, 0, 0).unwrap()
    }

    fn before_cutover() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2026, 10, 9, 23, 59, 59).unwrap()
    }

    fn after_cutover() -> DateTime<Utc> {
      Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()
    }
  }

  /// Version 1 (gif) thumbnail info, for the path and CDN tests above.
  fn version_1() -> VideoThumbnailInfo {
    info(Some(1), Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap())
  }

  fn info(maybe_thumbnail_version: Option<u8>, created_at: DateTime<Utc>) -> VideoThumbnailInfo {
    VideoThumbnailInfo { maybe_thumbnail_version, created_at }
  }
}
