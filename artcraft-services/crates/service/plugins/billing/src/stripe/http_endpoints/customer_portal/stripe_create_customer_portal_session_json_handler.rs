use actix_web::web::Json;
use actix_web::{web, HttpRequest, HttpResponse};
use component_traits::traits::internal_user_lookup::InternalUserLookup;
use url_config::third_party_url_redirector::ThirdPartyUrlRedirector;
use user_traits_component::traits::internal_session_cache_purge::InternalSessionCachePurge;

use crate::stripe::http_endpoints::customer_portal::stripe_create_customer_portal_session_error::CreateCustomerPortalSessionError;
use crate::stripe::http_endpoints::customer_portal::stripe_create_customer_portal_session_shared::stripe_create_customer_portal_session_shared;
use crate::stripe::stripe_config::StripeConfig;
use crate::stripe::traits::internal_product_to_stripe_lookup::InternalProductToStripeLookup;

// =============== Request ===============

#[derive(Deserialize)]
pub struct JsonRequest {
  // TODO: In the future, perhaps obscure this with well-known slugs
  /// The stripe portal configuration ID.
  /// This controls which plans can be switched to.
  portal_config_id: Option<String>,
}

// =============== Success Response ===============

#[derive(Serialize)]
pub struct SuccessResponse {
  pub success: bool,
  pub stripe_portal_redirect_url: String,
}

pub async fn stripe_create_customer_portal_session_json_handler(
  http_request: HttpRequest,
  request: Json<JsonRequest>,
  stripe_config: web::Data<StripeConfig>,
  stripe_client: web::Data<stripe::Client>,
  url_redirector: web::Data<ThirdPartyUrlRedirector>,
  internal_product_to_stripe_lookup: web::Data<dyn InternalProductToStripeLookup>,
  internal_user_lookup: web::Data<dyn InternalUserLookup>,
  internal_session_cache_purge: web::Data<dyn InternalSessionCachePurge>,
) -> Result<HttpResponse, CreateCustomerPortalSessionError>
{
  let portal_config_id = request.portal_config_id
      .as_deref()
      .unwrap_or(&stripe_config.portal.default_portal_config_id)
      .to_string();

  // Best effort to delete Redis session cache
  internal_session_cache_purge.best_effort_purge_session_cache(&http_request);

  let redirect_url = stripe_create_customer_portal_session_shared(
    http_request,
    stripe_config,
    stripe_client,
    url_redirector,
    internal_product_to_stripe_lookup,
    internal_user_lookup,
    &portal_config_id,
  ).await?;

  let response = SuccessResponse {
    success: true,
    stripe_portal_redirect_url: redirect_url,
  };

  let body = serde_json::to_string(&response)
      .map_err(|_e| CreateCustomerPortalSessionError::ServerError)?;

  Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body))
}
