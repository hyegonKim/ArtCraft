use std::path::PathBuf;

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::api::media_bytes::MediaBytes;

/// Reference to an input 3D mesh file (e.g. GLB/OBJ/FBX) for mesh-to-mesh
/// models like part splitting and retopology.
#[derive(Clone, Debug)]
pub enum MeshRef {
  MediaFileToken(MediaFileToken),
  Url(String),
  /// A file on the local filesystem; read directly, never round-tripped
  /// through the ArtCraft cloud.
  LocalPath(PathBuf),
  /// Bytes the caller already holds (e.g. pasted media).
  Bytes(MediaBytes),
}
