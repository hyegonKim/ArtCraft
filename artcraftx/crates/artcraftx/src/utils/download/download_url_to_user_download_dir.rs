use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::data_dir::app_data_root::AppDataRoot;
use anyhow::anyhow;
use log::{error, info};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use url::Url;

pub async fn download_url_to_user_download_dir(
  url: &Url,
  app_data_root: &AppDataRoot,
  app_prefs: &AppPreferences,
) -> Result<PathBuf, ArtcraftXError> {

  let url_file_name = download_file_name_from_url(url)?;

  let download_directory = app_prefs
      .downloads
      .preferred_download_directory
      .download_directory(app_data_root);

  info!("Pushing path `{}` onto downloaded directory `{:?}`", url_file_name, download_directory);

  let download_filename = {
    let mut download_filename = download_directory.clone();
    download_filename.push(&url_file_name);
    download_filename
  };

  info!("Download filename: `{:?}`", download_filename);
  
  if download_filename == download_directory {
    return Err(ArtcraftXError::AnyhowError(anyhow!("Download filename resolved to directory: {:?}", download_filename)))?;
  }

  if download_filename.exists() {
    if download_filename.is_dir() {
      return Err(ArtcraftXError::AnyhowError(anyhow!("Download path exists and resolved to directory: {:?}", download_filename)));
    } else {
      return Err(ArtcraftXError::CannotDownloadFilePathAlreadyExists { path: download_filename }.into());
    }
  }

  info!("Downloading bytes from url: {:?}", url);

  let response = reqwest::get(url.clone()).await?;
  let response_bytes = response.bytes().await?;

  info!("Writing file: {:?}", download_filename);

  let mut file = OpenOptions::new()
      .create(true)
      .write(true)
      .truncate(true)
      .open(&download_filename)?;

  file.write_all(&response_bytes)?;

  Ok(download_filename)
}

/// Derive (and vet) the local filename to save a download under from its URL.
pub fn download_file_name_from_url(url: &Url) -> Result<String, ArtcraftXError> {
  let path_segments = url.path_segments()
      .ok_or_else(|| ArtcraftXError::AnyhowError(anyhow!("URL does not have path segments (1): {:?}", url)))?;

  let mut url_file_name = path_segments.last()
      .ok_or_else(|| ArtcraftXError::AnyhowError(anyhow!("URL does not have path segments (2): {:?}", url)))?
      .trim()
      .to_string();

  if !url_file_name.to_lowercase().starts_with("artcraft") {
    url_file_name = format!("artcraft_{}", url_file_name);
  }

  check_download_file_name(&url_file_name)?;

  Ok(url_file_name)
}

/// Vet a filename (from any source) before writing it into the download
/// directory.
// TODO: This likely needs more protections.
pub fn check_download_file_name(filename: &str) -> Result<(), ArtcraftXError> {
  if filename.trim().is_empty() {
    error!("Download filename is empty!");
    return Err(ArtcraftXError::BadDownloadFilename { path: "".into() }.into());
  }

  if !filename.is_ascii() {
    error!("Download filename is not completely ASCII: {:?}", filename);
    return Err(ArtcraftXError::BadDownloadFilename { path: filename.into() }.into());
  }

  if filename.contains("/")
      || filename.contains("..")
      || filename.contains("\\")
  {
    error!("Cannot download filename that has relative path components: {:?}", filename);
    return Err(ArtcraftXError::BadDownloadFilename { path: filename.into() }.into());
  }

  if filename.contains("'")
      || filename.contains("\"")
      || filename.contains("%")
      || filename.contains("<")
      || filename.contains(">")
      || filename.contains("|")
  {
    error!("Download filename has dangerous path components: {:?}", filename);
    return Err(ArtcraftXError::BadDownloadFilename { path: filename.into() }.into());
  }

  if filename.ends_with(".apk")
      || filename.ends_with(".app")
      || filename.ends_with(".bat")
      || filename.ends_with(".cmd")
      || filename.ends_with(".com")
      || filename.ends_with(".ps1")
      || filename.ends_with(".sh")
      || filename.ends_with(".so")
      || filename.ends_with("dll") // Don't even risk the '.'
      || filename.ends_with("exe") // Don't even risk the '.'
  {
    error!("Cannot download filename that resembles executable: {:?}", filename);
    return Err(ArtcraftXError::BadDownloadFilename { path: filename.into() }.into());
  }

  Ok(())
}
