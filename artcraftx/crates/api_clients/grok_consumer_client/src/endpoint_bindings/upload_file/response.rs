use serde::Deserialize;

/// The `/http/upload-file-v2/direct` response (2026-08-23):
///
/// ```json
/// {
///   "uploadId": "304e7eb0-7a7b-40af-afe5-4e28eeeb0270",
///   "fileMetadata": {
///     "fileMetadataId": "a0a1a02e-6236-41be-b213-eaa2ac7bd7f2",
///     "fileMimeType": "image/png",
///     "fileName": "moon_door.png",
///     "fileUri": "users/{user_id}/{file_id}/content",
///     "parsedFileUri": "",
///     "createTime": "2026-08-23T22:46:22.235616548Z",
///     "fileSource": "SELF_UPLOAD_FILE_SOURCE"
///   }
/// }
/// ```
#[derive(Deserialize)]
pub (super) struct GrokApiUploadFileResponse {
  #[serde(rename = "uploadId")]
  pub upload_id: Option<String>,

  #[serde(rename = "fileMetadata")]
  pub file_metadata: Option<GrokApiUploadFileMetadata>,
}

#[derive(Deserialize)]
pub (super) struct GrokApiUploadFileMetadata {
  /// The uploaded file id, used to reference the file later.
  #[serde(rename = "fileMetadataId")]
  pub file_metadata_id: Option<String>,

  /// Partial path to the media file, not a full URI.
  /// eg. `users/{user_id}/{file_id}/content`
  #[serde(rename = "fileUri")]
  pub file_uri: Option<String>,
}
