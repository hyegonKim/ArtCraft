use crate::plans::model_weights::evaluate::test_cases::test_search_term_zelda::test_search_term_zelda;
use crate::plans::model_weights::evaluate::test_cases::test_upvoted_models::test_upvoted_models;
use crate::plans::model_weights::evaluate::test_cases::test_use_count::test_use_count;
use elasticsearch::Elasticsearch;
use errors::AnyhowResult;

pub async fn evaluate_model_weights_search(client: &Elasticsearch) -> AnyhowResult<()> {
  // Test sorting filters
  test_upvoted_models(client).await?;
  test_use_count(client).await?;

  // Test search terms and boosting algorithm
  test_search_term_zelda(client).await?;

  // New tests

  Ok(())
}

