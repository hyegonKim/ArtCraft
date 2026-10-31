use std::str::FromStr;

use actix_web::{web, HttpRequest};
use component_traits::traits::internal_user_lookup::InternalUserLookup;
use log::error;
use stripe::{BillingPortalSession, CreateBillingPortalSession, CustomerId};
use url_config::third_party_url_redirector::ThirdPartyUrlRedirector;

use crate::stripe::http_endpoints::customer_portal::stripe_create_customer_portal_session_error::CreateCustomerPortalSessionError;
use crate::stripe::stripe_config::{FullUrlOrPath, StripeConfig};
use crate::stripe::traits::internal_product_to_stripe_lookup::InternalProductToStripeLookup;

pub async fn stripe_create_customer_portal_session_shared(
  http_request: HttpRequest,
  stripe_config: web::Data<StripeConfig>,
  stripe_client: web::Data<stripe::Client>,
  url_redirector: web::Data<ThirdPartyUrlRedirector>,
  internal_product_to_stripe_lookup: web::Data<dyn InternalProductToStripeLookup>,
  internal_user_lookup: web::Data<dyn InternalUserLookup>,
  portal_config_id: &str,
)
  -> Result<String, CreateCustomerPortalSessionError>
{
  let maybe_user_metadata = internal_user_lookup
      .lookup_user_from_http_request(&http_request)
      .await
      .map_err(|err| {
        error!("Error looking up user: {:?}", err);
        CreateCustomerPortalSessionError::ServerError // NB: This was probably *our* fault.
      })?;

  // NB: Our integration relies on an internal user token being present.
  let user_metadata = match maybe_user_metadata {
    None => return Err(CreateCustomerPortalSessionError::InvalidSession),
    Some(user_metadata) => user_metadata,
  };

  let stripe_customer_id = match user_metadata.maybe_existing_stripe_customer_id {
    Some(stripe_customer_id) => {
      CustomerId::from_str(&stripe_customer_id)
          .map_err(|err| {
            error!("Problem constructing user's stripe customer id: {:?}", err);
            CreateCustomerPortalSessionError::ServerError // NB: This was probably *our* fault.
          })?
    }
    None => {
      error!("No stripe customer ID to create a portal with");
      return Err(CreateCustomerPortalSessionError::InvalidSession);
    }
  };

  let mut params = CreateBillingPortalSession::new(stripe_customer_id);

  let return_url = match &stripe_config.portal.return_url {
    FullUrlOrPath::FullUrl(url) => url.to_string(),
    FullUrlOrPath::Path(path) => url_redirector.frontend_redirect_url_for_path(&http_request, path)
        .map_err(|_| CreateCustomerPortalSessionError::ServerError)?,
  };

  params.return_url = Some(&return_url);
  params.configuration = Some(portal_config_id);

  let response = BillingPortalSession::create(stripe_client.as_ref(), params)
      .await
      .map_err(|e| {
        error!("Stripe billing portal error: {:?}", e);
        CreateCustomerPortalSessionError::StripeError
      })?;

  Ok(response.url.to_string()) // NB: Redirect URL
}
