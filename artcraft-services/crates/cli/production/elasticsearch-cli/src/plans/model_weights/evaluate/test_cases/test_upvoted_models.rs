use crate::plans::model_weights::evaluate::asserts::{assert_results_contain_tokens, assert_results_do_not_contain_tokens};
use crate::plans::model_weights::evaluate::print_debugging::print_titles_and_rating_count;
use elasticsearch::Elasticsearch;
use elasticsearch_schema::searches::search_model_weights::search_model_weights::{search_model_weights, ModelWeightsSortDirection, ModelWeightsSortField, SearchArgs};
use errors::AnyhowResult;

const UPVOTED_MODELS: [&str; 3] = [
  "weight_ppqs5038bvkm6wc29w0xfebzy", // Donald Trump : 132
  "weight_ahxbf2104ngsgyegncaefyy6j", // Plankton v2 (Doug Lawrence) : 128
  "weight_7jk8mgwkzsycqrxmfw5q4245y", // Homer Simpson: 115
];

pub async fn test_upvoted_models(
  client: &Elasticsearch
) -> AnyhowResult<()> {

  let results = search_model_weights(SearchArgs {
    search_term: "", // No search term
    maybe_creator_user_token: None,
    maybe_ietf_primary_language_subtag: None,
    maybe_weights_categories: None,
    maybe_weights_types: None,
    sort_field: Some(ModelWeightsSortField::PositiveRatingCount),
    sort_direction: None,
    minimum_score: None,
    client: &client,
  }).await?;

  print_titles_and_rating_count(&results);

  assert_results_contain_tokens(&results, &UPVOTED_MODELS)?;

  // Now invert, and we shouldn't expect popular tokens to be found
  let results = search_model_weights(SearchArgs {
    search_term: "", // No search term
    maybe_creator_user_token: None,
    maybe_ietf_primary_language_subtag: None,
    maybe_weights_categories: None,
    maybe_weights_types: None,
    sort_field: Some(ModelWeightsSortField::PositiveRatingCount),
    sort_direction: Some(ModelWeightsSortDirection::Ascending), // Least popular first
    minimum_score: None,
    client: &client,
  }).await?;

  assert_results_do_not_contain_tokens(&results, &UPVOTED_MODELS)?;

  Ok(())
}

