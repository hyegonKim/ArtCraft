use utoipa::ToSchema;

/// Simple stats that can be attached to any entity
#[derive(Serialize, Clone, ToSchema)]
pub struct SimpleEntityStats {
  /// Number of positive ratings (or "likes") for this item
  pub positive_rating_count: u32,
  /// Number of bookmarks for this item
  pub bookmark_count: u32,
}
