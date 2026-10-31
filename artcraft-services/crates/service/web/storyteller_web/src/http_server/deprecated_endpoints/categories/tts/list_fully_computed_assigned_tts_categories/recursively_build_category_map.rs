//// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use tokens::tokens::model_categories::ModelCategoryToken;

use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::query_and_construct_payload::{CategoryInfoLite, CategoryTokenToCategoryMap, CategoryTokenToModelTokensMap, ModelTokenToCategoryTokensMap};

fn recursive_category_tokens(model_category_map: &ModelTokenToCategoryTokensMap, all_categories: &Vec<CategoryInfoLite>) -> BTreeSet<ModelCategoryToken> {
  let mut category_tokens = leaf_category_tokens(model_category_map);

  let all_categories_by_token = all_categories.iter()
      .map(|cat| (cat.category_token.clone(), cat.clone()))
      .collect::<HashMap<ModelCategoryToken, CategoryInfoLite>>();

  let mut last_size = 0;

  // NB: Not really "recursive" :P
  while last_size != category_tokens.len() {
    last_size = category_tokens.len();

    let mut new_category_tokens = HashSet::new();

    for category_token in category_tokens.iter() {
      let maybe_parent_category_token= all_categories_by_token.get(category_token)
          .and_then(|category| category.maybe_parent_category_token.as_ref());

      let parent_category_token = match maybe_parent_category_token {
        Some(parent_category_token) => parent_category_token,
        None => continue,
      };

      if !category_tokens.contains(parent_category_token) {
        new_category_tokens.insert(parent_category_token.clone());
      }
    }

    if !new_category_tokens.is_empty() {
      category_tokens.extend(new_category_tokens);
    }
  }

  category_tokens
}

fn leaf_category_tokens(model_category_map: &ModelTokenToCategoryTokensMap) -> BTreeSet<ModelCategoryToken> {
  model_category_map
      .values()
      .flatten()
      .map(|category_token| category_token.clone())
      .collect::<BTreeSet<ModelCategoryToken>>()
}

fn leaf_category_to_model_map(model_category_map: &ModelTokenToCategoryTokensMap) -> CategoryTokenToModelTokensMap {
  let mut category_token_to_model_tokens : CategoryTokenToModelTokensMap = BTreeMap::new();

  for (model_token, category_tokens) in model_category_map.iter() {
    for category_token in category_tokens {
      // FIXME(bt, 2023-01-13): Cleanup overly verbose implementation
      if !category_token_to_model_tokens.contains_key(category_token) {
        category_token_to_model_tokens.insert(category_token.clone(), BTreeSet::new());
      }
      if let Some(inner_map) = category_token_to_model_tokens.get_mut(category_token) {
        inner_map.insert(model_token.clone());
      }
    }
  }

  category_token_to_model_tokens
}

pub fn recursive_category_to_model_map(model_category_map: &ModelTokenToCategoryTokensMap, all_categories: &Vec<CategoryInfoLite>) -> CategoryTokenToModelTokensMap {
  let category_token_to_category_map = all_categories.iter()
      .map(|category| {
        (category.category_token.clone(), category.clone())
      })
      .collect::<HashMap<ModelCategoryToken, CategoryInfoLite>>();

//  // Build a map of category => all ancestor categories.
//  let category_token_to_all_category_parent_tokens_map = all_categories.iter()
//      .map(|category| {
//        let category_token = category.category_token.clone();
//        let parent_category_tokens = find_category_ancestors(&category_token, &category_token_to_category_map);
//        (category_token, parent_category_tokens)
//      })
//      .collect::<HashMap<ModelCategoryToken, HashSet<ModelCategoryToken>>>();

  let mut category_token_to_model_tokens = BTreeMap::new();

  for (model_token, model_category_tokens) in model_category_map.iter() {

    for direct_category_token in model_category_tokens {
      // FIXME(bt, 2023-01-13): Cleanup overly verbose implementation

      let mut all_ancestor_categories = find_category_ancestors(direct_category_token, &category_token_to_category_map);
      all_ancestor_categories.insert(direct_category_token.clone());

      for category_token in all_ancestor_categories.iter() {

        if !category_token_to_model_tokens.contains_key(category_token) {
          category_token_to_model_tokens.insert(category_token.clone(), BTreeSet::new());
        }
        if let Some(inner_map) = category_token_to_model_tokens.get_mut(category_token) {
          inner_map.insert(model_token.clone());
        }
      }
    }
  }

  category_token_to_model_tokens
}

fn find_category_ancestors(category_token: &ModelCategoryToken, token_to_category_map: &CategoryTokenToCategoryMap) -> HashSet<ModelCategoryToken> {
  recursively_find_category_ancestors(category_token, token_to_category_map)
}

fn recursively_find_category_ancestors(
  category_token: &ModelCategoryToken,
  token_to_category_map: &CategoryTokenToCategoryMap,
) -> HashSet<ModelCategoryToken> {
  match token_to_category_map.get(category_token) {
    None => HashSet::new(),
    Some(category) => {
      match category.maybe_parent_category_token {
        None => HashSet::from([category.category_token.clone()]),
        Some(ref parent_category_token) => {
          let mut tokens = recursively_find_category_ancestors(parent_category_token, token_to_category_map);
          tokens.insert(category.category_token.clone());
          tokens
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  // TODO: This really needs tests!
}
