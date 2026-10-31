use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

use actix_helpers::route_builder::RouteBuilder;

use crate::http_server::endpoints::inference_job::delete::dismiss_finished_session_jobs_handler::dismiss_finished_session_jobs_handler;
use crate::http_server::endpoints::inference_job::delete::terminate_inference_job_handler::terminate_inference_job_handler;
use crate::http_server::endpoints::inference_job::get::batch_get_inference_job_status_handler::batch_get_inference_job_status_handler;
use crate::http_server::endpoints::inference_job::get::get_inference_job_status_handler::get_inference_job_status_handler;
use crate::http_server::endpoints::inference_job::list::list_session_jobs_handler::list_session_jobs_handler;
use crate::http_server::endpoints::inference_job::stats::get_pending_inference_job_count_handler::get_pending_inference_job_count_handler;

pub fn add_job_routes<T, B> (app: App<T>) -> App<T>
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
  let mut app =
      app.service(
        web::scope("/v1/jobs")
            .service(
              web::resource("/job/{token}")
                  .route(web::get().to(get_inference_job_status_handler))
                  .route(web::delete().to(terminate_inference_job_handler))
                  .route(web::head().to(|| HttpResponse::Ok()))
            )
            .service(
              web::resource("/batch")
                  .route(web::get().to(batch_get_inference_job_status_handler))
                  .route(web::head().to(|| HttpResponse::Ok()))
            )
            .service(
              web::resource("/session")
                  .route(web::get().to(list_session_jobs_handler))
                  .route(web::head().to(|| HttpResponse::Ok()))
            )
            .service(
              web::resource("/session/dismiss_finished")
                  .route(web::post().to(dismiss_finished_session_jobs_handler))
                  .route(web::head().to(|| HttpResponse::Ok()))
            )
      );

  // NB(bt): Old routes (these must be supported for AI streamers).
  let mut app = RouteBuilder::from_app(app)
      .add_get("/v1/model_inference/job_status/{token}", get_inference_job_status_handler)
      .add_delete("/v1/model_inference/job/{token}", terminate_inference_job_handler, true)
      // NB: This is a legacy endpoint. There's a better comprehensive job status endpoint.
      .add_get("/v1/model_inference/queue_length", get_pending_inference_job_count_handler)
      .into_app();

  app
}
