use std::collections::HashMap;

use log::info;
use seedance2pro_client::creds::seedance2pro_session::Seedance2ProSession;
use seedance2pro_client::requests::prepare_file_upload::prepare_file_upload::{prepare_file_upload, PrepareFileUploadArgs};
use seedance2pro_client::requests::upload_file::upload_file::{upload_file, UploadFileArgs};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::errors::provider_error::ProviderError;
use crate::utils::media_source_ref::{
  extension_from_name_hint, resolve_media_source_bytes, MediaSourceRef, ResolvedMediaBytes,
};

/// Resolves one image source to bytes (token → CDN download, URL → download,
/// local path → disk read, bytes → as-is) and re-uploads it to the
/// Seedance2Pro/Kinovi CDN. Returns the public URL.
///
/// This is the image-side twin of `generate_video::providers::kinovi::upload`.
/// The actual upload path doesn't care whether the file is a video or an
/// image, but keeping the two helpers separate lets each side evolve
/// independently and makes intent clear at call sites.
pub(crate) async fn upload_source_to_seedance2pro(
  session: &Seedance2ProSession,
  source: MediaSourceRef,
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
) -> Result<String, ArtcraftRouterError> {
  let ResolvedMediaBytes { bytes, maybe_name_hint, description, .. } =
      resolve_media_source_bytes(source, maybe_map).await?;

  let extension = maybe_name_hint.as_deref()
      .and_then(extension_from_name_hint)
      .unwrap_or_else(|| "jpg".to_string());

  info!("Uploading image reference to Seedance2Pro from {} ({} bytes, .{})", description, bytes.len(), extension);

  let prepare_response = prepare_file_upload(PrepareFileUploadArgs {
    session,
    extension,
    host_override: None,
  })
    .await
    .map_err(|err| ArtcraftRouterError::Provider(ProviderError::Seedance2Pro(err)))?;

  let upload_response = upload_file(UploadFileArgs {
    upload_url: prepare_response.upload_url,
    file_bytes: bytes,
    host_override: None,
  })
    .await
    .map_err(|err| ArtcraftRouterError::Provider(ProviderError::Seedance2Pro(err)))?;

  Ok(upload_response.public_url)
}
