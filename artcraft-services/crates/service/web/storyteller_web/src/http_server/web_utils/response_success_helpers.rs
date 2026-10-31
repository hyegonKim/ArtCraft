use actix_web::HttpResponse;
use artcraft_api_defs::common::responses::simple_generic_json_success::SimpleGenericJsonSuccess;

pub fn simple_json_success() -> HttpResponse {
  let response = SimpleGenericJsonSuccess {
    success: true,
  };

  let body = serde_json::to_string(&response)
      .unwrap_or("{ \"success\": true }".to_string());

  HttpResponse::Ok()
      .content_type("application/json")
      .body(body)
}
