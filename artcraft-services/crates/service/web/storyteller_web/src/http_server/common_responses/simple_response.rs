use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct SimpleResponse {
  pub success: bool,
}
