use crate::searches::search_model_weights::search_model_weights::{ModelWeightsSortDirection, ModelWeightsSortField};
use serde_json::{json, Value};

pub fn add_sort(
  mut query: Value,
  sort_field: Option<ModelWeightsSortField>,
  sort_direction: Option<ModelWeightsSortDirection>,
) -> Value {
  let sort_field = sort_field.unwrap_or_default();
  let sort_direction = sort_direction.unwrap_or_default();

  if sort_field == ModelWeightsSortField::MatchScore
      && sort_direction == ModelWeightsSortDirection::Descending {
    return query;
  }

  let sort = match sort_field {
    ModelWeightsSortField::CreatedAt => json!([
      {
        "created_at": {
          "order": sort_direction.to_str(),
        }
      },
      "_score"
    ]),
    ModelWeightsSortField::UsageCount => json!([
      {
        "cached_usage_count": {
          "order": sort_direction.to_str(),
        }
      },
      "_score"
    ]),
    ModelWeightsSortField::BookmarkCount => json!([
      {
        "bookmark_count": {
          "order": sort_direction.to_str(),
        }
      },
      "_score"
    ]),
    ModelWeightsSortField::PositiveRatingCount => json!({
      "ratings_positive_count": {
        "order": sort_direction.to_str(),
      }
    }),
    _ => return query,
  };

  if let Some(mut object) = query.as_object_mut() {
    object.insert("sort".to_string(), sort);
  }

  query
}

