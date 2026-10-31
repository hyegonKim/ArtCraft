use utoipa::ToSchema;

/// Pagination by page id
/// This type of pagination is by "page id" and "page count".
/// This should never be used to walk the entire public database as competitors and investors
/// can use it to glean information about the scale of our service.
/// This is best used for user profiles and scoped down results.
///
/// See `PaginationCursors` for the other type of pagination.
#[derive(Serialize, ToSchema)]
pub struct PaginationPage {
  /// The current page number
  pub current: usize,

  /// How many pages there are in total
  pub total_page_count: usize,
}

#[cfg(test)]
mod tests {
  use crate::http_server::common_responses::pagination_page::PaginationPage;

  #[test]
  fn test_serialize() {
    let pagination = PaginationPage {
      current: 5,
      total_page_count: 123,

    };

    let json = serde_json::to_string(&pagination).expect("serialization");

    assert_eq!(json, "{\"current\":5,\"total_page_count\":123}")
  }
}
