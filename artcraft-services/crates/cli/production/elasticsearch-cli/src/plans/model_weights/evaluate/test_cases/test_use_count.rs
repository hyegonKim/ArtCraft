use crate::plans::model_weights::evaluate::asserts::{assert_results_contain_tokens, assert_results_do_not_contain_tokens};
use elasticsearch::Elasticsearch;
use elasticsearch_schema::searches::search_model_weights::search_model_weights::{search_model_weights, ModelWeightsSortDirection, ModelWeightsSortField, SearchArgs};
use errors::AnyhowResult;

const POPULAR_VOICES : [&str; 3] = [
  "weight_hz7g8f1j4psrsw2sv67e4y61q", // Mariano Closs (Relator de fútbol Argentino)
  "weight_rhfg4chgrp42bnha8kqfrtmcq", // Mariano Closs (full version)
  "weight_mbcr352wfb1eq76tpy3ef3kx1", // Lionel Messi
];

pub async fn test_use_count(
  client: &Elasticsearch
) -> AnyhowResult<()> {

  let results = search_model_weights(SearchArgs {
    search_term: "", // No search term
    maybe_creator_user_token: None,
    maybe_ietf_primary_language_subtag: None,
    maybe_weights_categories: None,
    maybe_weights_types: None,
    sort_field: Some(ModelWeightsSortField::UsageCount),
    sort_direction: None,
    minimum_score: None,
    client: &client,
  }).await?;

  assert_results_contain_tokens(&results, &POPULAR_VOICES)?;

  // Now invert, and we shouldn't expect popular tokens to be found
  let results = search_model_weights(SearchArgs {
    search_term: "", // No search term
    maybe_creator_user_token: None,
    maybe_ietf_primary_language_subtag: None,
    maybe_weights_categories: None,
    maybe_weights_types: None,
    sort_field: Some(ModelWeightsSortField::UsageCount),
    sort_direction: Some(ModelWeightsSortDirection::Ascending), // Least popular first
    minimum_score: None,
    client: &client,
  }).await?;

  assert_results_do_not_contain_tokens(&results, &POPULAR_VOICES)?;

  Ok(())
}

