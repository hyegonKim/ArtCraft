// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use std::sync::Arc;

use actix_web::web::{Json, Query};
use actix_web::{web, HttpRequest};

use crate::http_server::endpoints::weights::search::search_model_weights_impl::{search_model_weights_impl, SearchModelWeightsError, SearchModelWeightsRequest, SearchModelWeightsSuccessResponse};
use crate::state::server_state::ServerState;

/// Search model weights (HTTP GET)
#[utoipa::path(
  get,
  tag = "Model Weights",
  path = "/v1/weights/search",
  responses(
    (status = 200, description = "Successful search", body = SearchModelWeightsSuccessResponse),
    (status = 500, description = "Server error", body = SearchModelWeightsError),
  ),
  params(
    ("request" = SearchModelWeightsRequest, description = "Payload for Request"),
  )
)]
pub async fn search_model_weights_http_get_handler(
  http_request: HttpRequest,
  request: Query<SearchModelWeightsRequest>,
  server_state: web::Data<Arc<ServerState>>
) -> Result<Json<SearchModelWeightsSuccessResponse>, SearchModelWeightsError>
{
  search_model_weights_impl(http_request, request.0, server_state).await
}
