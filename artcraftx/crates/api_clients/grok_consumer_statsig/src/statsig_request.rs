use serde::{Deserialize, Serialize};

/// The `x-statsig-id` binds the HTTP method and path it was minted for
/// (`msg = method!path!…`), so a signature for one endpoint is not valid for
/// another. A `StatsigRequest` is therefore the cache key: one minted signature
/// per `(method, path)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatsigRequest {
  /// Uppercase HTTP method, e.g. `"POST"`.
  pub method: String,

  /// Request path only (no origin / query), e.g.
  /// `"/rest/app-chat/conversations/new"`.
  pub path: String,
}

impl StatsigRequest {
  /// Build a request, normalizing the method to uppercase and trimming both
  /// fields so `(" post ", …)` and `("POST", …)` share a cache entry.
  pub fn new(method: impl AsRef<str>, path: impl Into<String>) -> Self {
    Self {
      method: method.as_ref().trim().to_ascii_uppercase(),
      path: path.into().trim().to_string(),
    }
  }

  /// The endpoint that starts every chat / media-generation stream, including
  /// video generation. This is the signature the video binding needs.
  pub fn new_conversation() -> Self {
    Self::new("POST", "/rest/app-chat/conversations/new")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_normalizes_method_and_trims() {
    let request = StatsigRequest::new(" post ", "  /rest/app-chat/conversations/new  ");
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/rest/app-chat/conversations/new");
  }

  #[test]
  fn equal_requests_hash_and_compare_equal() {
    assert_eq!(StatsigRequest::new("post", "/p"), StatsigRequest::new("POST", "/p"));
  }
}
