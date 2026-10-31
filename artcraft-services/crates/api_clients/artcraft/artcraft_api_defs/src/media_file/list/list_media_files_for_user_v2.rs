use serde_derive::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::media_file::list::user_media_file_list_item::MediaFileForUserListItem;

pub const LIST_MEDIA_FILES_FOR_USER_V2_URL_PATH: &str = "/v1/media_files/list_v2/user/{username}";

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ListMediaFilesForUserV2PathInfo {
  pub username: String,
}

#[derive(Serialize, Deserialize, ToSchema, IntoParams)]
pub struct ListMediaFilesForUserV2QueryParams {
  pub sort_ascending: Option<bool>,
  pub page_size: Option<usize>,
  pub page_index: Option<usize>,

  /// NB: This can be one (or more comma-separated values) from `MediaFileClass`,
  /// which are the broad category of media files: image, video, etc.
  ///
  /// Usage:
  ///   - `?filter_media_classes=audio`
  ///   - `?filter_media_classes=image,video`
  ///   - `?filter_media_classes=dimensional`
  ///   - etc.
  pub filter_media_classes: Option<String>,

  /// NB: This can be one (or more comma-separated values) from `MediaFileType`,
  /// which are mimetype-like / format-like categories of media files: glb, gltf,
  /// scene_json, jpg, png, mp4, wav, etc.
  ///
  /// Usage:
  ///   - `?filter_media_type=scene_json`
  ///   - `?filter_media_type=glb,gltf,fbx`
  ///   - `?filter_media_type=pmd,vmd,pmx`
  ///   - `?filter_media_type=jpg,png,gif`
  ///   - `?filter_media_type=wav,mp3`
  ///   - `?filter_media_type=mp4`
  ///   - etc.
  pub filter_media_type: Option<String>,

  /// NB: This can be one (or more comma-separated values) from `MediaFileEngineCategory`.
  ///
  /// Usage:
  ///   - `?filter_engine_categories=scene`
  ///   - `?filter_engine_categories=animation,character,object`
  ///   - etc.
  pub filter_engine_categories: Option<String>,

  /// Include user uploaded files in the results.
  /// By default, we do not return them unless this flag is set to true.
  pub include_user_uploads: Option<bool>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ListMediaFilesForUserV2SuccessResponse {
  pub success: bool,
  pub results: Vec<MediaFileForUserListItem>,
  pub pagination: UserMediaFileV2Pagination,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserMediaFileV2Pagination {
  pub current: usize,
  pub has_more: bool,
}
