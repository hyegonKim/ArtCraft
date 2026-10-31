//! `PUT <upload_url>` — send the file's bytes to the presigned storage URL
//! from a presign response. Not a gateway endpoint: no auth or host, the
//! signature in the URL is the credential.

use crate::client::send_request::send_presigned_upload;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;

pub struct UploadMediaBytesArgs<'a> {
  pub request: UploadMediaBytesRequest,

  /// The capturing browser's UA, for the same reason as on the gateway
  /// (the presign was issued to it).
  pub maybe_user_agent: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct UploadMediaBytesRequest {
  /// `PresignedMediaUpload::upload_url`.
  pub upload_url: String,

  /// Must match the type the slot was presigned for.
  pub content_type: MediaMimeType,

  pub bytes: Vec<u8>,
}

impl UploadMediaBytesRequest {
  pub fn for_slot(slot: &PresignedMediaUpload, bytes: Vec<u8>) -> Self {
    Self { upload_url: slot.upload_url.clone(), content_type: slot.content_type.clone(), bytes }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if !self.upload_url.starts_with("https://") {
      return Err(HiggsfieldClientError::InvalidRequest("upload_url is not an https URL".to_string()));
    }
    if self.bytes.is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("no bytes to upload".to_string()));
    }
    Ok(())
  }
}

/// Upload the bytes. Storage answers an empty `200`; the gateway doesn't
/// know about the file until `confirm_media_upload`.
pub async fn upload_media_bytes(args: UploadMediaBytesArgs<'_>) -> Result<(), HiggsfieldError> {
  args.request.validate()?;
  let UploadMediaBytesRequest { upload_url, content_type, bytes } = args.request;
  send_presigned_upload(&upload_url, content_type.as_str(), bytes, args.maybe_user_agent).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::ids::MediaId;

  fn slot() -> PresignedMediaUpload {
    PresignedMediaUpload {
      id: MediaId::new("m1"),
      url: "https://cdn.example.com/user_x/m1.png".to_string(),
      upload_url: "https://bucket.s3.amazonaws.com/user_x/m1.png?X-Amz-Signature=abc".to_string(),
      content_type: MediaMimeType::ImagePng,
      thumbnail_url: None,
    }
  }

  #[test]
  fn request_takes_type_and_url_from_the_slot() {
    let request = UploadMediaBytesRequest::for_slot(&slot(), vec![1, 2, 3]);
    assert_eq!(request.content_type, MediaMimeType::ImagePng);
    assert!(request.upload_url.contains("X-Amz-Signature"));
    assert!(request.validate().is_ok());
  }

  #[test]
  fn validation() {
    let empty = UploadMediaBytesRequest::for_slot(&slot(), vec![]);
    assert!(matches!(empty.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));

    let mut plain_http = UploadMediaBytesRequest::for_slot(&slot(), vec![1]);
    plain_http.upload_url = "http://bucket.s3.amazonaws.com/x".to_string();
    assert!(matches!(plain_http.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn rejects_before_network() {
    let err = upload_media_bytes(UploadMediaBytesArgs {
      request: UploadMediaBytesRequest::for_slot(&slot(), vec![]),
      maybe_user_agent: None,
    }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }
}
