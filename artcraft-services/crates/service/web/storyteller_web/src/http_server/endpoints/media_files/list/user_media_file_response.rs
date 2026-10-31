use artcraft_api_defs::common::responses::cover_image_links::CoverImageLinks;
use artcraft_api_defs::common::responses::legacy_media_file_cover_image_details::LegacyMediaFileCoverImageDetails;
use artcraft_api_defs::common::responses::media_file_cover_image_details::MediaFileDefaultCover;
use artcraft_api_defs::common::responses::media_file_origin_details::{
  MediaFileModelDetails as MediaFileModelDetailsApi, MediaFileOriginDetails as MediaFileOriginDetailsApi,
};
use artcraft_api_defs::common::responses::simple_entity_stats::SimpleEntityStats;
use artcraft_api_defs::media_file::list::user_media_file_list_item::MediaFileForUserListItem;
use bucket_paths::legacy::typified_paths::public::media_files::bucket_file_path::MediaFileBucketPath;
use enums::by_table::media_files::media_file_type::MediaFileType;
use mysql_queries::queries::media_files::list::user_media_file_list_item::MediaFileListItem;
use server_environment::ServerEnvironment;

use crate::http_server::common_responses::media::media_domain::MediaDomain;
use crate::http_server::common_responses::media::media_file_cover_image_details::MediaFileCoverImageDetails;
use crate::http_server::common_responses::media::media_links_builder::MediaLinksBuilder;
use crate::http_server::common_responses::media::media_links_builder::VideoThumbnailInfo;
use crate::http_server::common_responses::media_file_origin_details::{MediaFileModelDetails, MediaFileOriginDetails};
use crate::http_server::web_utils::bucket_urls::bucket_url_string_from_media_path::bucket_url_string_from_media_path;

pub(super) struct UserMediaFileResponseArgs {
  pub records: Vec<MediaFileListItem>,
  pub is_allowed_studio_access: bool,
  pub media_domain: MediaDomain,
  pub server_environment: ServerEnvironment,
}

/// Shared media-item representation; pagination contracts remain versioned.
pub(super) fn build_user_media_file_response(args: UserMediaFileResponseArgs) -> Vec<MediaFileForUserListItem> {
  let UserMediaFileResponseArgs {
    records,
    is_allowed_studio_access,
    media_domain,
    server_environment,
  } = args;
  records
    .into_iter()
    .filter(|record| {
      if is_allowed_studio_access {
        return true;
      }
      // Don't allow access to certain media types.
      match record.media_type {
        MediaFileType::Bvh
        | MediaFileType::Fbx
        | MediaFileType::Glb
        | MediaFileType::Gltf
        | MediaFileType::SceneRon => return false,
        _ => {}
      }
      // // Don't allow access to certain products.
      // match record.origin_product_category {
      //   MediaFileOriginProductCategory::VideoFilter |
      //   MediaFileOriginProductCategory::Mocap |
      //   MediaFileOriginProductCategory::Workflow => return false,
      //   _ => {},
      // }
      true
    })
    .map(|record| {
      let public_bucket_path = MediaFileBucketPath::from_object_hash(
        &record.public_bucket_directory_hash,
        record.maybe_public_bucket_prefix.as_deref(),
        record.maybe_public_bucket_extension.as_deref(),
      );
      MediaFileForUserListItem {
        token: record.token.clone(),
        media_class: record.media_class,
        media_type: record.media_type,
        maybe_engine_category: record.maybe_engine_category,
        maybe_animation_type: record.maybe_animation_type,
        origin: into_api_origin(MediaFileOriginDetails::from_db_fields_str(
          record.origin_category,
          record.origin_product_category,
          record.maybe_origin_model_type,
          record.maybe_origin_model_token.as_deref(),
          record.maybe_origin_model_title.as_deref(),
        )),
        origin_category: record.origin_category,
        origin_product_category: record.origin_product_category,
        maybe_origin_model_type: record.maybe_origin_model_type,
        maybe_origin_model_token: record.maybe_origin_model_token,
        media_links: MediaLinksBuilder::from_media_path_and_env(
          media_domain,
          server_environment,
          &public_bucket_path,
          VideoThumbnailInfo::new(record.maybe_thumbnail_version, record.created_at)),
        maybe_prompt_token: record.maybe_prompt_token,
        public_bucket_path: public_bucket_path.get_full_object_path_str().to_string(),
        public_bucket_url: bucket_url_string_from_media_path(&public_bucket_path, media_domain, server_environment),
        cover_image: into_api_cover_image(MediaFileCoverImageDetails::from_optional_db_fields(
          &record.token,
          media_domain,
          server_environment,
          record.maybe_file_cover_image_public_bucket_hash.as_deref(),
          record.maybe_file_cover_image_public_bucket_prefix.as_deref(),
          record.maybe_file_cover_image_public_bucket_extension.as_deref(),
        )),
        creator_set_visibility: record.creator_set_visibility,
        is_user_upload: record.is_user_upload,
        is_intermediate_system_file: record.is_intermediate_system_file,
        maybe_title: record.maybe_title,
        maybe_text_transcript: record.maybe_text_transcript,
        maybe_style_name: record
          .maybe_prompt_args
          .as_ref()
          .and_then(|args| args.style_name.as_ref())
          .and_then(|style| style.to_style_name()),
        maybe_duration_millis: record.maybe_duration_millis,
        stats: SimpleEntityStats {
          positive_rating_count: record.maybe_ratings_positive_count.unwrap_or(0),
          bookmark_count: record.maybe_bookmark_count.unwrap_or(0),
        },
        created_at: record.created_at,
        updated_at: record.updated_at,
      }
    })
    .collect::<Vec<_>>()
}

fn into_api_origin(origin: MediaFileOriginDetails) -> MediaFileOriginDetailsApi {
  MediaFileOriginDetailsApi {
    origin_category: origin.origin_category,
    product_category: origin.product_category,
    maybe_model: origin.maybe_model.map(|model| match model {
      MediaFileModelDetails::SystemModel { model_type } => MediaFileModelDetailsApi::SystemModel { model_type },
      MediaFileModelDetails::ModelWeight { model_type, token, title } => {
        MediaFileModelDetailsApi::ModelWeight { model_type, token, title }
      }
    }),
  }
}

fn into_api_cover_image(cover: MediaFileCoverImageDetails) -> LegacyMediaFileCoverImageDetails {
  LegacyMediaFileCoverImageDetails {
    maybe_cover_image_public_bucket_path: cover.maybe_cover_image_public_bucket_path,
    maybe_cover_image_public_bucket_url: cover.maybe_cover_image_public_bucket_url,
    maybe_links: cover.maybe_links.map(|links| CoverImageLinks {
      cdn_url: links.cdn_url,
      thumbnail_template: links.thumbnail_template,
    }),
    default_cover: MediaFileDefaultCover {
      image_index: cover.default_cover.image_index,
      color_index: cover.default_cover.color_index,
    },
  }
}

#[cfg(test)]
mod tests {
  use enums::by_table::media_files::media_file_origin_category::MediaFileOriginCategory;
  use enums::by_table::media_files::media_file_origin_model_type::MediaFileOriginModelType;
  use enums::by_table::media_files::media_file_origin_product_category::MediaFileOriginProductCategory;
  use tokens::tokens::media_files::MediaFileToken;

  use super::*;

  mod conversions {
    use super::*;

    #[test]
    fn origin_details_preserve_system_and_user_models() {
      for (model_type, model_token, model_title) in [
        (None, None, None),
        (Some(MediaFileOriginModelType::Tacotron2), None, None),
        (Some(MediaFileOriginModelType::Tacotron2), Some("weight_fixture"), Some("Fixture model")),
      ] {
        let origin = MediaFileOriginDetails::from_db_fields_str(
          MediaFileOriginCategory::Upload,
          MediaFileOriginProductCategory::Unknown,
          model_type,
          model_token,
          model_title,
        );
        let original = serde_json::to_value(&origin).unwrap();
        let converted = serde_json::to_value(into_api_origin(origin)).unwrap();
        assert_eq!(converted, original);
        let decoded: MediaFileOriginDetailsApi = serde_json::from_value(converted).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), original);
      }
    }

    #[test]
    fn cover_details_preserve_legacy_fields_and_cdn_links() {
      for maybe_hash in [None, Some("fixture_cover_hash")] {
        let cover = MediaFileCoverImageDetails::from_optional_db_fields(
          &MediaFileToken::new_from_str("m_fixture"),
          MediaDomain::FakeYou,
          ServerEnvironment::Production,
          maybe_hash,
          Some("image_"),
          Some(".png"),
        );
        let original = serde_json::to_value(&cover).unwrap();
        let converted = serde_json::to_value(into_api_cover_image(cover)).unwrap();
        assert_eq!(converted, original);
        let decoded: LegacyMediaFileCoverImageDetails = serde_json::from_value(converted).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), original);
      }
    }
  }
}
