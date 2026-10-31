use primitives::traits::trim_or_emptyable::TrimOrEmptyable;
use tokens::tokens::media_files::MediaFileToken;

// NB: Frontend is mistakenly sending empty string tokens - ignore those
pub fn empty_media_file_token_to_null(maybe_token: Option<&MediaFileToken>) -> Option<MediaFileToken> {
  maybe_token
      .filter(|t| t.as_str().trim_or_empty().is_some())
      .map(|t| t.clone())
}

#[cfg(test)]
mod tests {
  use tokens::tokens::media_files::MediaFileToken;

  use super::*;

  #[test]
  fn test_present() {
    let token = MediaFileToken::new_from_str("token");
    assert_eq!(empty_media_file_token_to_null(Some(&token)), Some(token));
  }

  #[test]
  fn test_empty() {
    let empty_token = MediaFileToken::new_from_str("");
    assert_eq!(empty_media_file_token_to_null(Some(&empty_token)), None);
  }
}