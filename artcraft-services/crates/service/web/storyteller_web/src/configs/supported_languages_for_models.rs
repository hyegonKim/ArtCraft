use language_tags::LanguageTag;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};

/// These are language tags we try to support.
static SUPPORTED_LANGUAGES_FOR_MODELS : Lazy<HashSet<String>> = Lazy::new(|| {
  let language_tags = [

    // ========== Technically, we only support these so far ==========

    // English
    "en",
    "en-AU",
    "en-CA",
    "en-GB",
    "en-US",

    // Spanish
    "es",
    "es-419",
    "es-AR",
    "es-CL",
    "es-CO",
    "es-ES",
    "es-MX",
    "es-PE",
    "es-US",

    // French
    "fr",
    "fr-CA",
    "fr-FR",

    // German
    "de",
    "de-AT", //	German (Austria)
    "de-CH", //	German (Switzerland)
    "de-DE", //	German (Germany)
    "de-LI", //	German (Liechtenstein)
    "de-LU", //	German (Luxembourg)

    // Hindi
    "hi",
    "hi-IN", // Hindi (India)

    // Italian
    "it",
    "it-CH", // https://en.wikipedia.org/wiki/Swiss_Italian
    "it-IT",

    // Portuguese
    "pt",
    "pt-BR",

    // Turkish
    "tr",
    "tr-TR", // Turkish (Turkey), which is more common amongst visitors than strictly "tr"

    // Arabic
    "ar",     // Arabic
    "ar-AE",	// Arabic (U.A.E.)
    "ar-BH",	// Arabic (Bahrain)
    "ar-DZ",	// Arabic (Algeria)
    "ar-EG",	// Arabic (Egypt)
    "ar-IQ",	// Arabic (Iraq)
    "ar-JO",	// Arabic (Jordan)
    "ar-KW",	// Arabic (Kuwait)
    "ar-LB",	// Arabic (Lebanon)
    "ar-LY",	// Arabic (Libya)
    "ar-MA",	// Arabic (Morocco)
    "ar-OM",	// Arabic (Oman)
    "ar-QA",	// Arabic (Qatar)
    "ar-SA",	// Arabic (Saudi Arabia)
    "ar-SY",	// Arabic (Syria)
    "ar-TN",	// Arabic (Tunisia)
    "ar-YE",	// Arabic (Yemen)

    // ========== But these are on the horizon ==========

    // Japanese
    "ja",
    "ja-JP",

    // Misc
    "id",
    "id-ID",
    "ru",
    "ru-RU",
    "th-TH",
    "tr",
    "tr-TR",
    "zh-CN",
    "zh-HK",
  ];
  language_tags.iter()
      .map(|tag| tag.to_string())
      .collect::<HashSet<String>>()
});

/// Convert lower case tags to canonical form
static SUPPORTED_LANGUAGES_FOR_MODELS_CANONICAL_MAP : Lazy<HashMap<String, String>> = Lazy::new(|| {
  SUPPORTED_LANGUAGES_FOR_MODELS.iter()
      .map(|tag| (tag.to_lowercase(), tag.clone()))
      .collect::<HashMap<_,_>>()
});

/// Take a full IETF BCP 47 language tag and return the canonicalized form *IF* we support it
pub fn get_canonicalized_language_tag_for_model(language_tag: &str) -> Option<&'static str> {
  let lowercase = language_tag.to_lowercase();
  SUPPORTED_LANGUAGES_FOR_MODELS_CANONICAL_MAP.get(&lowercase).map(|v| v.as_str())
}

/// Take a full IETF BCP 47 language tag and return true *IF* we support it
/// It must be the correct case.
pub fn is_valid_language_for_models(language_tag: &str) -> bool {
  SUPPORTED_LANGUAGES_FOR_MODELS.contains(language_tag)
}

/// Parse a language tag like "en-US" to "en".
pub fn get_primary_language_subtag(language_tag: &str) -> Option<String> {
  LanguageTag::parse(language_tag)
      .map(|language_tag| language_tag.primary_language().to_string())
      .ok()
}

#[cfg(test)]
mod tests {
  use crate::configs::supported_languages_for_models::{get_canonicalized_language_tag_for_model, get_primary_language_subtag, is_valid_language_for_models};

  #[test]
  fn get_canonical_language_for_model_success() {
    assert_eq!("en", get_canonicalized_language_tag_for_model("EN").unwrap());
    assert_eq!("es-419", get_canonicalized_language_tag_for_model("ES-419").unwrap());
    assert_eq!("en-US", get_canonicalized_language_tag_for_model("En-Us").unwrap());
    assert_eq!("it-IT", get_canonicalized_language_tag_for_model("iT-iT").unwrap());
  }

  #[test]
  fn get_canonical_language_for_model_failure() {
    assert!(get_canonicalized_language_tag_for_model("").is_none());
    assert!(get_canonicalized_language_tag_for_model("en-JP").is_none());
    assert!(get_canonicalized_language_tag_for_model("es-Lots-Of-Tags").is_none());
  }

  #[test]
  fn test_valid_language_for_model() {
    // Valid
    assert!(is_valid_language_for_models("en"));
    assert!(is_valid_language_for_models("es-419"));
    assert!(is_valid_language_for_models("ja-JP"));
    assert!(is_valid_language_for_models("it"));
    assert!(is_valid_language_for_models("it-CH"));
    // Invalid due to case
    assert!(!is_valid_language_for_models("EN"));
    assert!(!is_valid_language_for_models("ES-419"));
    assert!(!is_valid_language_for_models("JA-jp"));
    // Invalid misc
    assert!(!is_valid_language_for_models(""));
    assert!(!is_valid_language_for_models("foo"));
    assert!(!is_valid_language_for_models("en-FOO"));
  }

  #[test]
  fn test_primary_language_subtag() {
    assert_eq!(Some("en".to_string()), get_primary_language_subtag("en-US"));
    assert_eq!(Some("es".to_string()), get_primary_language_subtag("es-419"));
    assert_eq!(Some("it".to_string()), get_primary_language_subtag("it-CH"));
    assert_eq!(Some("ja".to_string()), get_primary_language_subtag("ja-JP"));
    assert_eq!(Some("zh".to_string()), get_primary_language_subtag("zh-CN"));
    assert_eq!(Some("zh".to_string()), get_primary_language_subtag("zh-HK"));
    assert_eq!(Some("tr".to_string()), get_primary_language_subtag("tr-TR"));
    assert_eq!(Some("ar".to_string()), get_primary_language_subtag("ar-EG"));
  }
}

