use crate::http_server::common_responses::media::media_domain::MediaDomain;
use crate::http_server::common_responses::media::media_links_builder::MediaLinksBuilder;
use crate::http_server::common_responses::media::media_links_builder::VideoThumbnailInfo;
use artcraft_api_defs::common::responses::media_links::MediaLinks;
use bucket_paths::legacy::typified_paths::public::media_files::bucket_file_path::MediaFileBucketPath;
use mysql_queries::queries::media_files::get::batch_get_media_files_by_tokens::MediaFilesByTokensRecord;
use server_environment::ServerEnvironment;

pub trait IntoMediaLinks {
  fn to_media_links(
    &self,
    media_domain: MediaDomain,
    server_environment: ServerEnvironment,
  ) -> MediaLinks;
}

impl IntoMediaLinks for MediaFilesByTokensRecord {
  fn to_media_links(&self, media_domain: MediaDomain, server_environment: ServerEnvironment) -> MediaLinks {
    let bucket_path = MediaFileBucketPath::from_object_hash(
      &self.public_bucket_directory_hash,
      self.maybe_public_bucket_prefix.as_deref(),
      self.maybe_public_bucket_extension.as_deref());
    MediaLinksBuilder::from_media_path_and_env(
      media_domain,
      server_environment,
      &bucket_path,
      VideoThumbnailInfo::new(self.maybe_thumbnail_version, self.created_at)
    )
  }
}
