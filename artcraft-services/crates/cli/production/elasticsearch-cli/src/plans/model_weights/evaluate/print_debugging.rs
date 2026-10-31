use std::collections::HashSet;
use elasticsearch_schema::documents::model_weight_document::ModelWeightDocument;

pub fn print_titles(results: &Vec<ModelWeightDocument>) {
  for result in results {
    println!("  - result: {:#?} ({:?})", result.title, result.token);
  }
}

pub fn print_titles_and_usage_counts(results: &Vec<ModelWeightDocument>) {
  for result in results {
    println!("  - result: {:#?} {} ({:?})",
             result.title,
             result.cached_usage_count.unwrap_or(0),
             result.token);
  }
}

pub fn print_titles_and_rating_count(results: &Vec<ModelWeightDocument>) {
  for result in results {
    println!("  - result: {:#?} {} ({:?})",
             result.title,
             result.ratings_positive_count,
             result.token);
  }
}

pub fn to_title_set(results: &Vec<ModelWeightDocument>) -> HashSet<String> {
  results.iter().map(|result| result.title.clone()).collect()
}

