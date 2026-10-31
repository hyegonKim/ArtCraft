use serde_derive::{Deserialize, Serialize};

use crate::api_defs::common::responses::pagination_cursors::PaginationCursors;
use crate::api_defs::media_file::list::by_type::list_session_common::SessionMediaFileInfo;

pub const LIST_SESSION_MESH_MEDIA_FILES_URL_PATH: &str = "/v1/media_files/mesh/list";

#[derive(Serialize, Deserialize)]
pub struct ListSessionMeshMediaFilesSuccessResponse {
  pub success: bool,
  pub results: Vec<SessionMediaFileInfo>,
  pub pagination: PaginationCursors,
}
