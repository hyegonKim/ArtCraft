use actix_web::HttpRequest;
use http_server_common::request::get_request_header_optional::get_request_header_optional;
use http_server_common::request::parse_accept_language::parse_accept_language;
use log::warn;
use utoipa::ToSchema;

pub const FORCE_LOCALE_COOKIE_HEADER_NAME : &str = "force-locale";

#[derive(Serialize, ToSchema)]
pub struct AppStateUserLocale {
  /// Full BCP47 language tags
  pub full_language_tags: Vec<String>,

  /// Parsed out languages
  pub language_codes: Vec<String>,
}

pub fn get_user_locale(
  http_request: &HttpRequest,
) -> AppStateUserLocale {
  let mut maybe_accept_language =
      get_request_header_optional(&http_request, "accept-language");

  if let Some(cookie) = http_request.cookie(FORCE_LOCALE_COOKIE_HEADER_NAME) {
    warn!("Overriding default accept language with custom value (from cookie)");
    maybe_accept_language = Some(cookie.value().to_string());

  } else if let Some(header) = get_request_header_optional(&http_request, FORCE_LOCALE_COOKIE_HEADER_NAME) {
    warn!("Overriding default accept language with custom value (from header)");
    maybe_accept_language = Some(header);
  }

  let accept_language = maybe_accept_language.unwrap_or("en".to_string());
  let language_tags = parse_accept_language(&accept_language);

  let mut  full_language_tags = Vec::new();
  let mut language_codes = Vec::new();

  for language_tag in language_tags.iter() {
    full_language_tags.push(language_tag.to_string());
    language_codes.push(language_tag.primary_language().to_string());
  }

  AppStateUserLocale {
    full_language_tags,
    language_codes,
  }
}