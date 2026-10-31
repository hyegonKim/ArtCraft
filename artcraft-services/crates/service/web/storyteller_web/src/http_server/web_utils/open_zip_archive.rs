use std::io::{BufReader, Cursor};

use log::{error, info, warn};
use zip::ZipArchive;

use mimetypes::mimetype_for_bytes::get_mimetype_for_bytes;

const ZIP_MIMETYPE : &str = "application/zip";

#[derive(Debug)]
pub enum OpenZipError {
  InvalidArchive,
  TooManyFiles,
}

pub fn open_zip_archive(
  zip_container_file_bytes: &[u8],
  maybe_max_file_count: Option<usize>,
) -> Result<ZipArchive<BufReader<Cursor<&[u8]>>>, OpenZipError>{

  let maybe_mimetype = get_mimetype_for_bytes(zip_container_file_bytes);

  if maybe_mimetype != Some(ZIP_MIMETYPE) {
    error!("File must be an application/zip");
    return Err(OpenZipError::InvalidArchive);
  }

  let mut cursor = Cursor::new(zip_container_file_bytes);
  let reader = BufReader::new(cursor);

  let mut archive = ZipArchive::new(reader)
      .map_err(|err| {
        error!("Problem reading zip archive: {:?}", err);
        OpenZipError::InvalidArchive
      })?;

  info!("Archive opened. Entry count: {}", archive.len());

  if let Some(max_file_count) = maybe_max_file_count {
    if archive.len() > max_file_count {
      warn!("Too many files in archive: {}", archive.len());
      return Err(OpenZipError::TooManyFiles);
    }
  }

  Ok(archive)
}
