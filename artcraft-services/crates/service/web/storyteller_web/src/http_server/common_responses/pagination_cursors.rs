use utoipa::ToSchema;

/// Pagination by cursors
/// These types of pagination are for "infinite scrolling", which do not reveal a number of pages.
/// This is good so that investors and competitors cannot reveal how many database records we have.
/// This is typically used for "discovery" type pages, not user profiles.
///
/// See `PaginationPage` for the other type of pagination.
#[derive(Serialize, ToSchema)]
pub struct PaginationCursors {

  /// The "next" cursor.
  /// This is an opaque (typically even encrypted) handle.
  pub maybe_next: Option<String>,

  /// The "previous" cursor.
  /// This is an opaque (typically even encrypted) handle.
  pub maybe_previous: Option<String>,

  /// Details whether we're walking forward or backward.
  pub cursor_is_reversed: bool,
}

#[cfg(test)]
mod tests {
  use crate::http_server::common_responses::pagination_cursors::PaginationCursors;

  #[test]
  fn test_serialize_empty_json() {
    // Make sure this interface doesn't change.
    let pagination = PaginationCursors {
      maybe_next: None,
      maybe_previous: None,
      cursor_is_reversed: false,
    };

    let json = serde_json::to_string(&pagination).expect("serialization");

    assert_eq!(json, "{\"maybe_next\":null,\"maybe_previous\":null,\"cursor_is_reversed\":false}")
  }

  #[test]
  fn test_serialize_full_json() {
    // Make sure this interface doesn't change.
    let pagination = PaginationCursors {
      maybe_next: Some("foo".to_string()),
      maybe_previous: Some("bar".to_string()),
      cursor_is_reversed: true,
    };

    let json = serde_json::to_string(&pagination).expect("serialization");

    assert_eq!(json, "{\"maybe_next\":\"foo\",\"maybe_previous\":\"bar\",\"cursor_is_reversed\":true}")
  }
}
