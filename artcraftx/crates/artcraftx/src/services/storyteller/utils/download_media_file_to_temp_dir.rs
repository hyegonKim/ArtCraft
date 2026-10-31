use artcraft_client::utils::api_host::ApiHost;
use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::get_url_file_extension::get_url_file_extension;
use crate::utils::download::simple_http_download_to_tempfile::simple_http_download_to_tempfile;
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use tempfile::NamedTempFile;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
// TODO: Better more concrete error handling

pub async fn download_media_file_to_temp_dir(app_data_root: &AppDataRoot, token: &MediaFileToken) -> Result<NamedTempFile, ArtcraftXError> {
  let response = get_media_file(
    &ApiHost::Storyteller, 
    token
  ).await?;

  let media_file_url = &response.media_file.media_links.cdn_url;

  let extension_with_dot = get_url_file_extension(media_file_url)
      .map(|ext| format!(".{}", ext))
      .unwrap_or_else(|| ".png".to_string()); // TODO: Better default extension passed by caller as heuristic.

  //let filename = format!("{}{}", response.media_file.token.as_str(), extension_with_dot);
  //let filename = app_data_root.downloads_dir().path().join(&filename);

  let mut file = app_data_root.temp_dir().new_named_temp_file_with_extension(&extension_with_dot)?;

  simple_http_download_to_tempfile(&media_file_url, &mut file).await?;

  Ok(file) // NB: Must return TempFile to not drop / delete it
}
