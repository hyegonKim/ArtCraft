use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use crate::utils::get_url_file_extension::get_url_file_extension;
use crate::utils::download::simple_http_download::simple_http_download;
use crate::services::grok::state::grok_credential_manager::GrokCredentialManager;
use anyhow::anyhow;
use errors::AnyhowResult;
use grok_consumer_client::datatypes::api::file_id::FileId;
use grok_consumer_client::endpoint_bindings::upload_file::grok_upload_file::{grok_upload_file, GrokUploadFileArgs, GrokUploadFileRequest, PathOrFile};
use log::info;
use std::time::Duration;
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use artcraft_client::utils::api_host::ApiHost;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

const GROK_IMAGE_UPLOAD_TIMEOUT: Duration = Duration::from_millis(1000 * 30); // 30 seconds

pub struct UploadImageMediaFileToGrok<'a> {
  pub storyteller_host: &'a ApiHost,
  pub image_media_token: &'a MediaFileToken,

  pub app_data_root: &'a AppDataRoot,
  pub grok_creds_manager: &'a GrokCredentialManager,
}

pub struct UploadImageMediaFileToGrokResult {
  pub grok_file_id: FileId,
}

pub async fn upload_image_media_file_to_grok(
  args: UploadImageMediaFileToGrok<'_>,
) -> AnyhowResult<UploadImageMediaFileToGrokResult> {

  info!("Calling get media file API: {:?}", args.storyteller_host);

  info!("Using media token: {:?}", args.image_media_token);

  let response = get_media_file(
    args.storyteller_host,
    args.image_media_token
  ).await?;

  let media_file_url = &response.media_file.media_links.cdn_url;
  let extension_with_dot = get_url_file_extension(media_file_url)
      .map(|ext| format!(".{}", ext))
      .unwrap_or_else(|| ".png".to_string());

  let filename = format!("{}{}", response.media_file.token.as_str(), extension_with_dot);
  let filename = args.app_data_root.downloads_dir().path().join(&filename);

  simple_http_download(&media_file_url, &filename).await?;

  let cookies = args.grok_creds_manager.maybe_copy_cookie_header_string()?
      .ok_or_else(|| anyhow!("Missing Grok cookies"))?;

  info!("Uploading image to Grok...");

  let response = grok_upload_file(GrokUploadFileArgs {
    request: GrokUploadFileRequest {
      file: PathOrFile::Path(filename.as_path()),
    },
    cookie: &cookies,
    domain_override: None,
    request_timeout: Some(GROK_IMAGE_UPLOAD_TIMEOUT),
  }).await?;

  let file_id = response.file_id
      .ok_or_else(|| anyhow!("Media upload did not produce a file_id!"))?;

  Ok(UploadImageMediaFileToGrokResult {
    grok_file_id: file_id,
  })
}
