use elasticsearch::Elasticsearch;
use elasticsearch_schema::documents::model_weight_document::ModelWeightDocument;
use elasticsearch_schema::searches::search_model_weights::search_model_weights::{search_model_weights, SearchArgs};
use errors::AnyhowResult;

pub async fn search_term_only(search_term: &str, client: &Elasticsearch) -> AnyhowResult<Vec<ModelWeightDocument>> {
  search_model_weights(SearchArgs {
    search_term,
    maybe_creator_user_token: None,
    maybe_ietf_primary_language_subtag: None,
    maybe_weights_categories: None,
    maybe_weights_types: None,
    sort_field: None,
    sort_direction: None,
    minimum_score: None,
    client: &client,
  }).await
}

