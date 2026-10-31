use crate::client::browser_user_agents::FIREFOX_143_MAC_USER_AGENT;
use crate::client::grok_domain::GrokDomain;
use crate::datatypes::api::file_id::FileId;
use crate::endpoint_bindings::upload_file::response::GrokApiUploadFileResponse;
use crate::error::grok_client_error::GrokClientError;
use crate::error::grok_error::GrokError;
use crate::error::grok_generic_api_error::GrokGenericApiError;
use log::{error, info};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE, COOKIE, ORIGIN, REFERER, USER_AGENT};
use wreq::multipart::{Form, Part};
use wreq::Client;
use wreq_util::Emulation;

/// The upload path (appended to the [`GrokDomain`]). As of 2026-08-23 this is a
/// `multipart/form-data` upload of the raw bytes (previously a JSON body with
/// base64 `content`).
const UPLOAD_FILE_PATH: &str = "/http/upload-file-v2/direct";

/// The multipart field name the web app uses for the file part.
const FILE_FIELD_NAME: &str = "file";

/// Try to prevent buffer reallocations.
const INITIAL_BUFFER_SIZE: usize = 1024 * 1024;

/// The file to upload: a path to read from disk, or in-memory bytes.
pub enum PathOrFile<'a> {
  /// A path to a file on disk. Its name and MIME type are derived from the path.
  Path(&'a Path),
  /// In-memory bytes with an explicit name and MIME type.
  File {
    bytes: Vec<u8>,
    file_name: String,
    mime_type: String,
  },
}

pub struct GrokUploadFileRequest<'a> {
  pub file: PathOrFile<'a>,
}

pub struct GrokUploadFileArgs<'a> {
  pub request: GrokUploadFileRequest<'a>,
  pub cookie: &'a str,
  pub domain_override: Option<&'a GrokDomain>,
  pub request_timeout: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct GrokUploadFileResponse {
  pub file_id: Option<FileId>,
  pub file_uri: Option<String>,
}

/// Upload a file to Grok.
pub async fn grok_upload_file(args: GrokUploadFileArgs<'_>) -> Result<GrokUploadFileResponse, GrokError> {
  let (bytes, file_name, mime_type) = match args.request.file {
    PathOrFile::Path(path) => read_file_for_upload(path).await?,
    PathOrFile::File { bytes, file_name, mime_type } => (bytes, file_name, mime_type),
  };

  let domain = args.domain_override.unwrap_or(&GrokDomain::DefaultDomain);

  do_upload(domain, args.cookie, args.request_timeout, bytes, file_name, mime_type).await
}

async fn do_upload(
  domain: &GrokDomain,
  cookie: &str,
  request_timeout: Option<Duration>,
  bytes: Vec<u8>,
  file_name: String,
  mime_type: String,
) -> Result<GrokUploadFileResponse, GrokError> {
  let client = Client::builder()
      .emulation(Emulation::Firefox143)
      .build()
      .map_err(GrokClientError::WreqClientError)?;

  info!("Uploading {} ({} bytes) to Grok...", file_name, bytes.len());

  // Raw bytes in a single `file` part. `.multipart()` sets the
  // `content-type: multipart/form-data; boundary=...` header itself.
  let part = Part::bytes(bytes)
      .file_name(file_name)
      .mime_str(&mime_type)
      .map_err(GrokClientError::WreqClientError)?;
  let form = Form::new().part(FILE_FIELD_NAME, part);

  let mut request_builder = client.post(request_url(domain))
      .header(ACCEPT, "*/*")
      .header(ACCEPT_LANGUAGE, "en-US,en;q=0.5")
      .header(COOKIE, cookie)
      .header(ORIGIN, "https://grok.com")
      .header(REFERER, "https://grok.com/imagine")
      .header("priority", "u=1, i")
      .header("sec-fetch-dest", "empty")
      .header("sec-fetch-mode", "cors")
      .header("sec-fetch-site", "same-origin")
      .header(USER_AGENT, FIREFOX_143_MAC_USER_AGENT)
      .multipart(form);

  if let Some(timeout) = request_timeout {
    request_builder = request_builder.timeout(timeout);
  }

  let http_request = request_builder
      .build()
      .map_err(|err| {
        error!("Error building image upload request: {:?}", err);
        GrokClientError::WreqClientError(err)
      })?;

  let response = client.execute(http_request)
      .await
      .map_err(|err| {
        error!("Error during image upload: {:?}", err);
        GrokGenericApiError::WreqError(err)
      })?;

  let status = response.status();

  let response_body = response.text()
      .await
      .map_err(|err| {
        error!("Error reading Grok image upload response body: {:?}", err);
        GrokGenericApiError::WreqError(err)
      })?;

  if !status.is_success() {
    error!("Upload file request returned an error (code {}): {:?}", status.as_u16(), response_body);
    // TODO: Categorize Cloudflare / auth / rate-limit errors.
  }

  parse_upload_response(&response_body)
}

fn request_url(domain: &GrokDomain) -> String {
  format!("{}{}", domain.get_domain(), UPLOAD_FILE_PATH)
}

/// Read a file into memory and derive its upload name and MIME type.
async fn read_file_for_upload(file_path: &Path) -> Result<(Vec<u8>, String, String), GrokError> {
  let mut file = File::open(file_path)
      .await
      .map_err(|err| {
        error!("Failed to open file for upload: {}", err);
        GrokClientError::CannotOpenLocalFileForUpload(err)
      })?;

  let mut buffer = Vec::with_capacity(INITIAL_BUFFER_SIZE);
  file.read_to_end(&mut buffer)
      .await
      .map_err(|err| {
        error!("Failed to read file for upload: {}", err);
        GrokClientError::CannotReadLocalFileForUpload(err)
      })?;

  let file_name = file_path
      .file_name()
      .ok_or(GrokClientError::FileForUploadHasInvalidPath)?
      .to_string_lossy()
      .to_string();

  let mime_type = mime_type_for_extension(file_path.extension().and_then(|e| e.to_str()));

  Ok((buffer, file_name, mime_type.to_string()))
}

/// Best-effort MIME type from a file extension.
// TODO: Read file magic bytes first, then fall back to this.
fn mime_type_for_extension(extension: Option<&str>) -> &'static str {
  match extension {
    Some("jpg") | Some("jpeg") => "image/jpeg",
    Some("png") => "image/png",
    Some("webp") => "image/webp",
    Some("gif") => "image/gif",
    _ => "application/octet-stream",
  }
}

/// Parse the upload response into the public [`GrokUploadFileResponse`]. The
/// file id and uri now live under `fileMetadata`.
fn parse_upload_response(body: &str) -> Result<GrokUploadFileResponse, GrokError> {
  let response: GrokApiUploadFileResponse = serde_json::from_str(body)
      .map_err(|err| GrokGenericApiError::SerdeResponseParseErrorWithBody(err, body.to_string()))?;

  let metadata = response.file_metadata;

  Ok(GrokUploadFileResponse {
    file_id: metadata.as_ref().and_then(|m| m.file_metadata_id.clone()).map(FileId),
    file_uri: metadata.and_then(|m| m.file_uri),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  // Wire-format tests below are deliberately image-free — they exercise the
  // URL, the MIME mapping, and response parsing without embedding a payload.

  mod wire_format_tests {
    use super::*;

    #[test]
    fn url_uses_the_default_domain() {
      assert_eq!(
        request_url(&GrokDomain::DefaultDomain),
        "https://grok.com/http/upload-file-v2/direct",
      );
    }

    #[test]
    fn url_respects_a_domain_override() {
      let domain = GrokDomain::Custom("http://localhost:8080".to_string());
      assert_eq!(request_url(&domain), "http://localhost:8080/http/upload-file-v2/direct");
    }

    #[test]
    fn mime_type_is_derived_from_extension() {
      assert_eq!(mime_type_for_extension(Some("jpg")), "image/jpeg");
      assert_eq!(mime_type_for_extension(Some("jpeg")), "image/jpeg");
      assert_eq!(mime_type_for_extension(Some("png")), "image/png");
      assert_eq!(mime_type_for_extension(Some("webp")), "image/webp");
      assert_eq!(mime_type_for_extension(None), "application/octet-stream");
      assert_eq!(mime_type_for_extension(Some("xyz")), "application/octet-stream");
    }

    #[test]
    fn parses_real_upload_response() {
      // Real response from 17_upload_image.txt, user id scrubbed.
      let body = std::fs::read_to_string("test_data/endpoint_responses/upload_file.json").unwrap();

      let response = parse_upload_response(&body).unwrap();

      assert_eq!(
        response.file_id.as_ref().map(|id| id.0.as_str()),
        Some("a0a1a02e-6236-41be-b213-eaa2ac7bd7f2"),
      );
      assert_eq!(
        response.file_uri.as_deref(),
        Some("users/00000000-0000-4000-8000-000000000000/a0a1a02e-6236-41be-b213-eaa2ac7bd7f2/content"),
      );
    }

    #[test]
    fn malformed_response_is_an_error() {
      let result = parse_upload_response("{ not json");
      assert!(matches!(
        result,
        Err(GrokError::ApiGeneric(GrokGenericApiError::SerdeResponseParseErrorWithBody(_, _))),
      ));
    }
  }

  mod real_wire_tests {
    use super::*;
    use crate::test_utils::grok_test_secrets::load_grok_test_secrets;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    use errors::AnyhowResult;
    use log::LevelFilter;

    // A real (non-trivial) PNG lives here — Grok's upload pipeline rejects
    // degenerate images. Cargo runs tests from the crate root.
    const TEST_IMAGE_PATH: &str = "test_data/images/test_upload.png";

    #[tokio::test]
    #[ignore] // Hits the real website; requires external/credentials/grok.
    async fn upload_file() -> AnyhowResult<()> {
      setup_test_logging(LevelFilter::Info);
      let secrets = load_grok_test_secrets()?;

      let result = grok_upload_file(GrokUploadFileArgs {
        request: GrokUploadFileRequest {
          file: PathOrFile::Path(Path::new(TEST_IMAGE_PATH)),
        },
        cookie: secrets.cookies.as_str(),
        domain_override: None,
        request_timeout: Some(Duration::from_secs(30)),
      }).await?;

      println!("Upload result: {:?}", result);

      assert!(result.file_id.is_some(), "expected a file id from the upload");
      assert!(result.file_uri.is_some(), "expected a file uri from the upload");
      Ok(())
    }
  }
}
