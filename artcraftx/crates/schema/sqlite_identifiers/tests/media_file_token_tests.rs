/// Test MediaFileToken in isolation, just in case our macro-derived tests break.

use serde::Deserialize;
use serde::Serialize;

use sqlite_identifiers::ids::media_file_token::MediaFileToken;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct CompositeType {
  media_file_token: MediaFileToken,
  string: String,
}

mod interface {
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  #[test]
  fn generate() {
    let token = MediaFileToken::generate();
    assert!(!token.to_string().is_empty());
    assert!(token.to_string().starts_with("m_"));
  }

  #[test]
  fn generated_tokens_have_stable_format() {
    for _ in 0..25 {
      let token = MediaFileToken::generate();
      let value = token.as_str();
      assert_eq!(value.len(), 32, "token {value} should be 32 chars");
      assert!(value.starts_with("m_"));
      assert!(value[2..].bytes().all(|b| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&b)),
        "token {value} should be lowercase crockford");
    }
  }

  #[test]
  fn new() {
    let token = MediaFileToken::new("m_foo".to_string());
    assert_eq!(token, MediaFileToken("m_foo".to_string()));
  }

  #[test]
  fn new_from_str() {
    let token = MediaFileToken::new_from_str("m_foo");
    assert_eq!(token, MediaFileToken("m_foo".to_string()));
  }

  #[test]
  fn as_str() {
    let token = MediaFileToken("m_foo".to_string());
    assert_eq!(token.as_str(), "m_foo");
  }

  #[test]
  fn to_string() {
    let token = MediaFileToken("m_foo".to_string());
    assert_eq!(token.to_string(), "m_foo".to_string());
  }
}

mod traits {
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  #[test]
  fn display() {
    let token = MediaFileToken("m_foo".to_string());
    assert_eq!(format!("{}", token), "m_foo".to_string());
  }

  #[test]
  fn debug() {
    let token = MediaFileToken("m_foo".to_string());
    assert_eq!(format!("{:?}", token), "MediaFileToken(\"m_foo\")".to_string());
  }
}

mod serialization {
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  use crate::CompositeType;

// NB(bt,2024-09-25): These tests are broken with a recent upgrade
//  #[test]
//  fn serialize() {
//    let expected = "\"m_foo\"".to_string(); // NB: Quoted
//
//    let token = MediaFileToken("m_foo".to_string());
//    assert_eq!(expected, toml::to_string(&token).unwrap());
//
//    // Just to show this serializes the same as a string
//    assert_eq!(expected, toml::to_string("m_foo").unwrap());
//  }

  #[test]
  fn nested_serialize() {
    let value = CompositeType { media_file_token: MediaFileToken("m_foo".to_string()), string: "bar".to_string() };
    let expected = r#"{"media_file_token":"m_foo","string":"bar"}"#.to_string();
    assert_eq!(expected, serde_json::to_string(&value).unwrap());
  }
}

mod deserialization {
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  use crate::CompositeType;

  #[test]
  fn deserialize() {
    let payload = "\"m_foo\""; // NB: Quoted
    let expected = "m_foo".to_string();

    let value: MediaFileToken = serde_json::from_str(payload).unwrap();
    assert_eq!(value, MediaFileToken(expected.clone()));

    // Just to show this deserializes the same way as a string
    let value: String = serde_json::from_str(payload).unwrap();
    assert_eq!(value, expected.clone());
  }

  #[test]
  fn nested_deserialize() {
    let payload = r#"{"media_file_token":"m_foo","string":"bar"}"#.to_string();
    let expected = CompositeType {
      media_file_token: MediaFileToken("m_foo".to_string()),
      string: "bar".to_string(),
    };

    assert_eq!(expected, serde_json::from_str::<CompositeType>(&payload).unwrap());
  }
}

// These traits should be tested by the macro, but we duplicate them in case that breaks
mod crockford_traits {
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  const ENTROPIC_CHARACTERS_MINIMUM : usize = 8;

  #[test]
  fn entropy_is_sufficient() {
    assert!((32 - MediaFileToken::PREFIX.len()) > ENTROPIC_CHARACTERS_MINIMUM);
  }

  #[test]
  fn token_length() {
    assert_eq!(MediaFileToken::generate().as_str().len(), 32);
  }

  #[test]
  fn tokens_are_random() {
    let mut tokens = std::collections::HashSet::new();
    for _ in 0..100 {
      tokens.insert(MediaFileToken::generate().to_string());
    }
    assert_eq!(tokens.len(), 100);
  }

  #[test]
  fn character_set() {
    let token_string = MediaFileToken::generate().to_string();
    let prefix = MediaFileToken::PREFIX;
    let random_part = token_string.replace(prefix, "");

    assert!(random_part.len() > ENTROPIC_CHARACTERS_MINIMUM);
    assert!(random_part.chars().all(|c| c.is_numeric() || c.is_lowercase()));
  }

  #[test]
  fn prefix_ends_with_separator() {
    let prefix = MediaFileToken::PREFIX;
    assert!(prefix.ends_with("_"));

    let token_string = MediaFileToken::generate().to_string();
    assert!(token_string.contains(":") || token_string.contains("_"));
  }

  #[test]
  fn token_begins_with_prefix() {
    let prefix = MediaFileToken::PREFIX;
    let token_string = MediaFileToken::generate().to_string();
    assert!(token_string.starts_with(prefix));
  }

  #[test]
  fn entropy_suffix() {
    let token = MediaFileToken::new_from_str("m_foo");
    assert_eq!(token.entropy_suffix(), "foo");

    let token = MediaFileToken::new_from_str("bar");
    assert_eq!(token.entropy_suffix(), "bar");

    let token = MediaFileToken::generate();
    assert_eq!(token.entropy_suffix().len(), 30);
  }
}
