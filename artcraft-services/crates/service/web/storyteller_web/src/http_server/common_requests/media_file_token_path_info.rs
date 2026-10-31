use tokens::tokens::media_files::MediaFileToken;
use utoipa::ToSchema;

/// Media File Token in URL PathInfo
#[derive(Deserialize, ToSchema)]
pub struct MediaFileTokenPathInfo {
  pub token: MediaFileToken,
}
