use std::collections::HashMap;

use seedance2pro_client::creds::seedance2pro_session::Seedance2ProSession;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::image_list_ref::ImageListRef;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::providers::kinovi::upload::upload_source_to_seedance2pro;
use crate::utils::media_source_ref::MediaSourceRef;

/// Resolve an [`ImageListRef`] to a list of public Seedance2Pro CDN URLs,
/// uploading each image one at a time. Preserves order.
///
/// Returns `Ok(None)` when the input is `None` or an empty list — the
/// caller can then omit the field from the wire request entirely. (The
/// downstream Midjourney API rejects empty arrays.)
pub(crate) async fn resolve_and_upload_image_list(
  session: &Seedance2ProSession,
  list: Option<ImageListRef>,
  maybe_map: Option<&HashMap<MediaFileToken, String>>,
) -> Result<Option<Vec<String>>, ArtcraftRouterError> {
  let sources = match list {
    None => return Ok(None),
    Some(list) if list.is_empty() => return Ok(None),
    Some(list) => MediaSourceRef::list_from_images(list),
  };

  let mut uploaded = Vec::with_capacity(sources.len());
  for source in sources {
    uploaded.push(upload_source_to_seedance2pro(session, source, maybe_map).await?);
  }
  Ok(Some(uploaded))
}
