use crate::plans::model_weights::evaluate::print_debugging::{print_titles, to_title_set};
use crate::plans::model_weights::evaluate::search::search_term_only;
use elasticsearch::Elasticsearch;
use elasticsearch_schema::documents::model_weight_document::ModelWeightDocument;
use errors::{anyhow, AnyhowResult};
use log::error;
use std::collections::HashSet;
use tokens::tokens::model_weights::ModelWeightToken;

pub async fn assert_search_term_contains_titles(
  client: &Elasticsearch,
  search_term: &str,
  expected_titles: &Vec<&str>,
) -> AnyhowResult<()> {
  let results = search_term_only(search_term, client).await?;
  let titles = to_title_set(&results);

  println!("Titles:");
  print_titles(&results);

  if let Err(err) = assert_contains_all(&titles, expected_titles, search_term) {
    error!("Title not found for search: {}", search_term);
    print_titles(&results);
    return Err(err);
  }

  Ok(())
}
pub fn assert_contains(titles: &HashSet<String>, expected: &str, search_term: &str) -> AnyhowResult<()> {
  if !titles.contains(expected) {
    error!("Expected title not found: {} for search {} ({} results)",
      expected, search_term, titles.len());

    return Err(anyhow!("Expected title not found: {} for search {} ({} results)",
      expected, search_term, titles.len()));
  }
  Ok(())
}

pub fn assert_contains_all(titles: &HashSet<String>, expected: &Vec<&str>, search_term: &str) -> AnyhowResult<()> {
  for title in expected.iter() {
    assert_contains(titles, title, search_term)?;
  }
  Ok(())
}

pub fn assert_results_contain_tokens(
  results: &Vec<ModelWeightDocument>,
  expected_tokens: &[&str],
) -> AnyhowResult<()> {
  let tokens = results.iter()
      .map(|result| result.token.clone())
      .collect::<HashSet<_>>();

  for token in expected_tokens.iter() {
    let token = ModelWeightToken::new_from_str(token);
    if !tokens.contains(&token) {
      error!("Expected token not found: {}", token);
      return Err(anyhow!("Expected token not found: {}", token));
    }
  }

  Ok(())
}

pub fn assert_results_do_not_contain_tokens(
  results: &Vec<ModelWeightDocument>,
  expected_tokens: &[&str],
) -> AnyhowResult<()> {
  let tokens = results.iter()
      .map(|result| result.token.clone())
      .collect::<HashSet<_>>();

  for token in expected_tokens.iter() {
    let token = ModelWeightToken::new_from_str(token);
    if tokens.contains(&token) {
      error!("Unexpected token found: {}", token);
      return Err(anyhow!("Unexpected token found: {}", token));
    }
  }

  Ok(())
}
