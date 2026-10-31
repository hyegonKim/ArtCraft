use language_tags::LanguageTag;
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;

/// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Language
static ACCEPT_LANGUAGE_QUALITY_FACTOR_REGEX : Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#";q=\d+\.\d+"#).expect("should be valid regex")
});

/// Parse out accept languages
/// Does not error so that the endpoint won't degrade
pub fn parse_accept_language(accept_languages_header: &str) -> Vec<LanguageTag> {
  let unparsed_tags = accept_languages_header.split(",")
      .into_iter()
      .map(|tag| tag.trim())
      .map(|tag| ACCEPT_LANGUAGE_QUALITY_FACTOR_REGEX.replace(tag, ""))
      .map(|tag| tag.to_string())
      .collect::<Vec<String>>();

  let mut parsed_tags = Vec::new();

  for tag in unparsed_tags.iter() {
    match LanguageTag::parse(tag.as_ref()) {
      Ok(t) => {
        parsed_tags.push(t);
      }
      Err(e) => {
        warn!("Error parsing language tag '{}' : {:?}", tag, e);
      }
    }
  }

  parsed_tags
}

#[cfg(test)]
mod tests {
  use crate::request::parse_accept_language::parse_accept_language;

  #[test]
  fn test_empty() {
    assert_eq!(parse_accept_language(""), Vec::new());
  }

  #[test]
  fn test_garbage() {
    assert_eq!(parse_accept_language("HJHKDJSHJKHSF"), Vec::new());
    assert_eq!(parse_accept_language("\n\n"), Vec::new());
  }

  #[test]
  fn test_single_language() {
    let list = parse_accept_language("en-US");
    let lang = list.get(0).unwrap();
    assert_eq!(lang.primary_language(), "en");

    let list = parse_accept_language("en-GB");
    let lang = list.get(0).unwrap();
    assert_eq!(lang.primary_language(), "en");

    let list = parse_accept_language("ja-JP");
    let lang = list.get(0).unwrap();
    assert_eq!(lang.primary_language(), "ja");
  }

  #[test]
  fn test_multiple() {
    let list = parse_accept_language("en-US, es-419");
    let first = list.get(0).unwrap();
    let second = list.get(1).unwrap();
    assert_eq!(first.primary_language(), "en");
    assert_eq!(second.primary_language(), "es");

    let list = parse_accept_language("en, es, en-GB");
    let first = list.get(0).unwrap();
    let second = list.get(1).unwrap();
    let third= list.get(2).unwrap();
    assert_eq!(first.primary_language(), "en");
    assert_eq!(second.primary_language(), "es");
    assert_eq!(third.primary_language(), "en");
  }

  #[test]
  fn test_language_with_q_factor() {
    let list = parse_accept_language("en;q=0.9");
    let lang = list.get(0).unwrap();
    assert_eq!(lang.primary_language(), "en");

    let list = parse_accept_language("fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5");
    let first = list.get(0).unwrap();
    let second = list.get(1).unwrap();
    let third = list.get(2).unwrap();
    let fourth = list.get(3).unwrap();
    assert_eq!(first.primary_language(), "fr");
    assert_eq!(second.primary_language(), "fr");
    assert_eq!(third.primary_language(), "en");
    assert_eq!(fourth.primary_language(), "de");

    // TODO: Handle wildcard?
  }

  #[test]
  fn vegito_browser() {
    // This is FakeYou user 'vegito's accept language
    let list = parse_accept_language("en-US,en;q=0.9,es;q=0.8");
    assert_eq!(list.get(0).unwrap().primary_language(), "en");
    assert_eq!(list.get(1).unwrap().primary_language(), "en");
    assert_eq!(list.get(2).unwrap().primary_language(), "es");
  }
}
