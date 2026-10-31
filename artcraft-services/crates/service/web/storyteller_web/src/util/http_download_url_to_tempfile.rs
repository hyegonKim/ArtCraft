use crate::util::http_download_url_to_bytes::http_download_url_to_bytes;
use errors::AnyhowResult;
use std::io::Write;
use tempfile::NamedTempFile;

const USER_AGENT: &str = "storyteller-web/1.0";

/// Downloads a (binary) file to a filesystem path. Good for images, etc. Not great for large files.
pub async fn http_download_url_to_tempfile(url: &str, temp_file: &mut NamedTempFile) -> AnyhowResult<()> {
  let bytes = http_download_url_to_bytes(url).await?;
  temp_file.write_all(&bytes)?;
  Ok(())
}
