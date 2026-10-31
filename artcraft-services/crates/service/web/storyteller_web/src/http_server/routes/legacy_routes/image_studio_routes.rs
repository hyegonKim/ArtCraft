use crate::http_server::endpoints::image_studio::update_gpt_image_job_status_handler::update_gpt_image_job_status_handler;
use crate::http_server::endpoints::image_studio::upload::upload_snapshot_media_file_handler::upload_snapshot_media_file_handler;
use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{web, App, Error, HttpResponse};

pub fn add_image_studio_routes<T, B> (app: App<T>) -> App<T>
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
  app.service(web::scope("/v1/image_studio")
    .service(web::resource("/scene_snapshot")
      .route(web::post().to(upload_snapshot_media_file_handler))
      .route(web::head().to(|| HttpResponse::Ok()))
    )
    .service(web::resource("/update_job_status")
      .route(web::post().to(update_gpt_image_job_status_handler))
      .route(web::head().to(|| HttpResponse::Ok()))
    )
  )
}
