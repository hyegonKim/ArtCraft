use mysql_queries::payloads::media_file_extra_info::media_file_extra_info::MediaFileExtraInfo;
use tokens::tokens::media_files::MediaFileToken;
use utoipa::ToSchema;

/// Details about submitted live portrait jobs (request arguments only)
#[derive(Serialize, ToSchema)]
pub struct MediaFileLivePortraitDetails {
  /// Source media token
  /// This can be an image or a video media file token
  /// This is what constitutes the "portrait" or the overall final video.
  /// This video or image must contain a face.
  pub source_media_file_token: MediaFileToken,

  /// Driving media token
  /// This must be a video media file token.
  /// This drives the animation of the face, but the actor will disappear
  /// and their facial expressions will be transferred to the source.
  /// This video must contain a face.
  pub face_driver_media_file_token: MediaFileToken,
}

impl MediaFileLivePortraitDetails {
  /// Extract from database JSON payload
  pub fn maybe_from_extra_info(info: &MediaFileExtraInfo) -> Option<Self> {
    let lp =
        if let MediaFileExtraInfo::L(live_portrait_args) = info {
          live_portrait_args
        } else {
          return None;
        };

    lp.maybe_portrait_media_token
        .as_ref()
        .zip(lp.maybe_driver_video_media_token.as_ref())
        .map(|(portrait, driver) | {
          Self {
            source_media_file_token: portrait.clone(),
            face_driver_media_file_token: driver.clone(),
          }
        })
  }
}
