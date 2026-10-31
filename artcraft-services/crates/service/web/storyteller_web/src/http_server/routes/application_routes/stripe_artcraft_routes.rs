use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};
use billing_artcraft_component::endpoints::checkout::stripe_artcraft_create_credits_pack_checkout_handler::stripe_artcraft_create_credits_pack_checkout_handler;
use billing_artcraft_component::endpoints::checkout::stripe_artcraft_create_subscription_checkout_handler::stripe_artcraft_create_subscription_session_handler;
use billing_artcraft_component::endpoints::checkout_with_user_signup::stripe_artcraft_create_subscription_checkout_with_user_signup_handler::stripe_artcraft_create_subscription_checkout_with_user_signup_handler;
use billing_artcraft_component::endpoints::customer_portal::stripe_artcraft_customer_portal_cancel_plan_handler::stripe_artcraft_customer_portal_cancel_plan_handler;
use billing_artcraft_component::endpoints::customer_portal::stripe_artcraft_customer_portal_manage_plan_handler::stripe_artcraft_customer_portal_manage_plan_handler;
use billing_artcraft_component::endpoints::customer_portal::stripe_artcraft_customer_portal_switch_plan_handler::stripe_artcraft_customer_portal_switch_plan_handler;
use billing_artcraft_component::endpoints::customer_portal::stripe_artcraft_customer_portal_update_payment_method_handler::stripe_artcraft_customer_portal_update_payment_method_handler;
use billing_artcraft_component::endpoints::webhook::stripe_artcraft_webhook_handler::stripe_artcraft_webhook_handler;

pub fn add_stripe_artcraft_routes<T, B> (app: App<T>) -> App<T>
where
    B: MessageBody,
    T: ServiceFactory<
      ServiceRequest,
      Config = (),
      Response = ServiceResponse<B>,
      Error = Error,
      InitError = (),
    >,
{
  app.service(web::scope("/v1/stripe_artcraft")
      .service(web::resource("/webhook")
          .route(web::post().to(stripe_artcraft_webhook_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/checkout/credits_pack")
          .route(web::post().to(stripe_artcraft_create_credits_pack_checkout_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/checkout/subscription")
          .route(web::post().to(stripe_artcraft_create_subscription_session_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/portal/cancel_plan")
          .route(web::post().to(stripe_artcraft_customer_portal_cancel_plan_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/portal/manage_plan")
          .route(web::post().to(stripe_artcraft_customer_portal_manage_plan_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/portal/switch_plan")
          .route(web::post().to(stripe_artcraft_customer_portal_switch_plan_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/portal/update_payment_method")
          .route(web::post().to(stripe_artcraft_customer_portal_update_payment_method_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
      .service(web::resource("/user_signup_subscription_checkout")
          .route(web::post().to(stripe_artcraft_create_subscription_checkout_with_user_signup_handler))
          .route(web::head().to(|| HttpResponse::Ok()))
      )
  )
}
