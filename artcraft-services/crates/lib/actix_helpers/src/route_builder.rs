use actix_http::body::MessageBody;
use actix_service::ServiceFactory;
use actix_web::{App, FromRequest, HttpResponse, Responder, web};
use actix_web::dev::{Handler, ServiceRequest, ServiceResponse};
use actix_web::error::Error;

// TODO: Check that routes don't overlap.
// TODO: Properly handle scoped routes in a nice DSL.
// TODO: Add tests.

/// Build routes more concisely.
pub struct RouteBuilder<T, B>
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
  app: App<T>,
}

impl <T, B> RouteBuilder<T, B>
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
  /// Constructor
  pub fn from_app(app: App<T>) -> Self {
    Self {
      app
    }
  }

  /// Return back to Actix App.
  pub fn into_app(self) -> App<T> {
    self.app
  }

  /// Add an HTTP GET route. This also adds the HEAD request for CORS.
  pub fn add_get<F, Args>(mut self, path: &str, handler: F) -> Self
    where
        F: Handler<Args>,
        Args: FromRequest + 'static,
        F::Output: Responder + 'static,
  {
    self.app = self.app.service(
      web::resource(path)
          .route(web::get().to(handler))
          .route(web::head().to(|| HttpResponse::Ok())) // NB: For XHR/CORS HEAD requests.
    );
    self
  }

  /// Add an HTTP POST route. This also adds the HEAD request for CORS.
  pub fn add_post<F, Args>(mut self, path: &str, handler: F) -> Self
    where
        F: Handler<Args>,
        Args: FromRequest + 'static,
        F::Output: Responder + 'static,
  {
    self.app = self.app.service(
      web::resource(path)
          .route(web::post().to(handler))
          .route(web::head().to(|| HttpResponse::Ok())) // NB: For XHR/CORS HEAD requests.
    );
    self
  }

  /// Add an HTTP DELETE route.
  pub fn add_delete<F, Args>(mut self, path: &str, handler: F, add_head_route: bool) -> Self
    where
        F: Handler<Args>,
        Args: FromRequest + 'static,
        F::Output: Responder + 'static,
  {
    if add_head_route {
      self.app = self.app.service(
        web::resource(path)
            .route(web::delete().to(handler))
            .route(web::head().to(|| HttpResponse::Ok())) // NB: For XHR/CORS HEAD requests.
      );
    } else {
      self.app = self.app.service(
        web::resource(path)
            .route(web::delete().to(handler))
      );
    }
    self
  }
}
