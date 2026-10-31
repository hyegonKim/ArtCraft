use serde_derive::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

use crate::common::responses::cover_image_links::CoverImageLinks;
use crate::common::responses::media_file_cover_image_details::MediaFileDefaultCover;

// User-media list endpoints retain the deprecated bucket fields for compatibility.
/// Everything we need to create a cover image.
/// Cover images are small descriptive images that can be set for any media file.
/// If a cover image is set, this is the path to the asset.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LegacyMediaFileCoverImageDetails {
  /// (DEPRECATED) URL path to the media file
  #[deprecated(note="This field doesn't point to the full URL. Use media_links instead to leverage the CDN.")]
  pub maybe_cover_image_public_bucket_path: Option<String>,

  /// (DEPRECATED) Full URL to the media file
  #[deprecated(note="This points to the bucket. Use media_links instead to leverage the CDN.")]
  pub maybe_cover_image_public_bucket_url: Option<Url>,

  /// Links to the cover image (CDN direct link, thumbnail template)
  /// If a cover image is set, this is the path to the asset.
  /// If a cover image is not set, use the information in `default_cover` instead.
  /// Rich CDN links to the media, including thumbnails, previews, and more.
  pub maybe_links: Option<CoverImageLinks>,

  /// For items without a cover image, we can use one of our own.
  pub default_cover: MediaFileDefaultCover,
}
