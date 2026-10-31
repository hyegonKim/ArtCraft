use std::collections::{BTreeMap, HashMap};

use lexical_sort::natural_lexical_cmp;

use tokens::tokens::model_categories::ModelCategoryToken;
use tokens::tokens::tts_models::TtsModelToken;

use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::query_and_construct_payload::{CategoryTokenToModelTokensMap, TtsModelInfoLite};

const UNNAMED_MODEL_SORT_VALUE : &str = "unnamed";

struct SortableModelToken {
  model_token: TtsModelToken,
  model_sortable_name: String,
}

pub fn sort_models(
  category_to_model_map: &CategoryTokenToModelTokensMap,
  models: &Vec<TtsModelInfoLite>
) -> BTreeMap<ModelCategoryToken, Vec<TtsModelToken>> {
  let models_by_token = models.into_iter()
      .map(|model| (model.model_token.clone(), model))
      .collect::<HashMap<TtsModelToken, &TtsModelInfoLite>>();

  let mut results = BTreeMap::new();

  for (category_token, model_tokens) in category_to_model_map.iter() {
    let mut sorted_model_tokens = model_tokens.iter()
        .map(|model_token| {
          let model_sortable_name = models_by_token.get(model_token)
              .map(|model| model.title_for_sorting.to_string())
              .unwrap_or(UNNAMED_MODEL_SORT_VALUE.to_string())
              .to_lowercase();

          SortableModelToken {
            model_token: model_token.clone(),
            model_sortable_name,
          }
        })
        .collect::<Vec<SortableModelToken>>();

    sorted_model_tokens.sort_by(|a, b|
        natural_lexical_cmp(&a.model_sortable_name, &b.model_sortable_name));

    let sorted_model_tokens = sorted_model_tokens.into_iter()
        .map(|token_info| token_info.model_token)
        .collect::<Vec<TtsModelToken>>();

    results.insert(category_token.clone(), sorted_model_tokens);
  }

  results
}
