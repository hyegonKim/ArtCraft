use tokens::tokens::media_files::MediaFileToken;
use utoipa::ToSchema;

/// Media File Token in URL PathInfo
#[derive(Deserialize, ToSchema)]
#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Path)]
pub struct MediaFileTokenPathInfo {
  pub token: MediaFileToken,
}
