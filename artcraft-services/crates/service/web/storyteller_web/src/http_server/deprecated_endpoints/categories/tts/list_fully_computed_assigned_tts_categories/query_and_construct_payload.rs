//// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};
use lexical_sort::natural_lexical_cmp;
use log::error;
use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool};

use errors::AnyhowResult;
use memory_caching::single_item_ttl_cache::SingleItemTtlCache;
use mysql_queries::queries::model_categories::list_categories_query_builder::CategoryList;
use mysql_queries::queries::trending_model_analytics::list_trending_tts_models::list_trending_tts_models;
use mysql_queries::queries::tts::tts_category_assignments::fetch_and_build_tts_model_category_map::fetch_and_build_tts_model_category_map_with_connection;
use mysql_queries::queries::tts::tts_models::list_tts_models::list_tts_models_with_connection;
use tokens::tokens::model_categories::ModelCategoryToken;
use tokens::tokens::tts_models::TtsModelToken;

use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::add_synthetic_categories::{add_recent_models, add_trending_models};
use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::error::ListFullyComputedAssignedTtsCategoriesError;
use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::list_fully_computed_assigned_tts_categories::ModelTokensByCategoryToken;
use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::recursively_build_category_map::recursive_category_to_model_map;
use crate::http_server::deprecated_endpoints::categories::tts::list_fully_computed_assigned_tts_categories::sort_models::sort_models;
use crate::state::cached_queries::list_cached_tts_categories_for_public_dropdown::list_cached_tts_categories_for_public_dropdown;

#[derive(Clone)]
pub struct CategoryInfoLite {
  pub category_token: ModelCategoryToken,
  pub maybe_parent_category_token: Option<ModelCategoryToken>,
  // Fields for sorting:
  pub category_name_for_sorting: String,
}

#[derive(Clone)]
pub struct TtsModelInfoLite {
  pub model_token: TtsModelToken,
  // Fields for sorting:
  pub title_for_sorting: String,
  pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TtsModelTokenToCategoryTokenLight {
  map: HashMap<TtsModelToken, HashSet<ModelCategoryToken>> // NB: Stronger types than the library we consume
}

pub type CategoryTokenToCategoryMap = HashMap<ModelCategoryToken, CategoryInfoLite>;

pub type ModelTokenToCategoryTokensMap = HashMap<TtsModelToken, HashSet<ModelCategoryToken>>;

// NB: We use BTree to maintain insertion order for our return type.
pub type CategoryTokenToModelTokensMap = BTreeMap<ModelCategoryToken, BTreeSet<TtsModelToken>>;

pub async fn query_and_construct_payload(
  category_cache: &SingleItemTtlCache<CategoryList>,
  mysql_pool: &MySqlPool
) -> Result<ModelTokensByCategoryToken, ListFullyComputedAssignedTtsCategoriesError> {
  let (
    categories,
    models,
    model_category_map,
    trending_models
  ) = {
    let mut mysql_connection = mysql_pool.acquire()
        .await
        .map_err(|e| {
          error!("Could not acquire DB pool: {:?}", e);
          ListFullyComputedAssignedTtsCategoriesError::ServerError
        })?;

    let models = list_tts_models(&mut mysql_connection)
        .await
        .map_err(|e| {
          error!("Error querying models: {:?}", e);
          ListFullyComputedAssignedTtsCategoriesError::ServerError
        })?;

    let categories = list_tts_categories(category_cache, &mut mysql_connection)
        .await
        .map_err(|e| {
          error!("Error querying categories: {:?}", e);
          ListFullyComputedAssignedTtsCategoriesError::ServerError
        })?;

    let model_category_map = build_model_categories_map(&mut mysql_connection)
        .await
        .map_err(|e| {
          error!("Error querying and building model category map: {:?}", e);
          ListFullyComputedAssignedTtsCategoriesError::ServerError
        })?;

    let trending_models = list_trending_tts_models(&mut mysql_connection)
        .await
        .map_err(|e| {
          error!("Error querying trending TTS models: {:?}", e);
          ListFullyComputedAssignedTtsCategoriesError::ServerError
        })?;

    (categories, models, model_category_map, trending_models)
  };

  let mut recursive_category_to_model_map = recursive_category_to_model_map(
    &model_category_map,
    &categories);

  // Add synthetic categories
  add_recent_models(&mut recursive_category_to_model_map, &models);
  add_trending_models(&mut recursive_category_to_model_map, trending_models);

  // TODO: This is a mess, because late in the process I realized "BTreeMap"/"BTreeSet" sorts on
  //  key and not insertion order. Classic reading the docs.

  // Sort all of the models within the categories
  let final_map = sort_models(&recursive_category_to_model_map, &models);

  Ok(ModelTokensByCategoryToken {
    recursive: final_map,
  })
}

// ========== Queries / model transformations ==========

async fn list_tts_categories(
  category_cache: &SingleItemTtlCache<CategoryList>,
  mysql_connection: &mut PoolConnection<MySql>
) -> AnyhowResult<Vec<CategoryInfoLite>> {

  let categories = list_cached_tts_categories_for_public_dropdown(category_cache, mysql_connection).await?;
  let categories = categories.categories;

  let mut categories = categories.into_iter()
      .map(|c| CategoryInfoLite {
        category_token: ModelCategoryToken::new(c.category_token),
        maybe_parent_category_token: c.maybe_super_category_token.map(|t| ModelCategoryToken::new(t)),
        // NB: This might produce weird sorting resorts relative to the "name" field,
        // but the typical way this should be consumed is via dropdowns.
        category_name_for_sorting: c.maybe_dropdown_name.unwrap_or(c.name),
      })
      .collect::<Vec<CategoryInfoLite>>();

  // NB: This might produce weird sorting resorts relative to the "name" field,
  // but the typical way this should be consumed is via dropdowns.
  categories.sort_by(|c1, c2|
      natural_lexical_cmp(&c1.category_name_for_sorting, &c2.category_name_for_sorting));

  Ok(categories)
}

async fn list_tts_models(mysql_connection: &mut PoolConnection<MySql>) -> AnyhowResult<Vec<TtsModelInfoLite>> {
  let models = list_tts_models_with_connection(
    mysql_connection,
    None,
    false
  ).await?;

  let mut models = models.into_iter()
      .map(|m| TtsModelInfoLite {
        model_token: TtsModelToken::new_from_str(&m.model_token),
        title_for_sorting: m.title,
        created_at: m.created_at,
      })
      .collect::<Vec<TtsModelInfoLite>>();

  // Make the list nice for human readers.
  models.sort_by(|a, b|
      natural_lexical_cmp(&a.title_for_sorting, &b.title_for_sorting));

  Ok(models)
}

async fn build_model_categories_map(mysql_connection: &mut PoolConnection<MySql>) -> AnyhowResult<ModelTokenToCategoryTokensMap> {
  // NB: It looks like the underlying code filters out TTS models if they're deleted or locked,
  // but it does no filtering (or joining!) to categories, which may result in spurious categories
  // being returned in the map.
  let untyped_map  = fetch_and_build_tts_model_category_map_with_connection(mysql_connection).await?;

  // NB: Stronger types
  let map = untyped_map.model_to_category_tokens.into_iter()
      .map(|(model_token, category_tokens)| {
        let model_token = TtsModelToken::new(model_token);
        let category_tokens = category_tokens.into_iter()
            .map(|category_token| ModelCategoryToken::new(category_token))
            .collect::<HashSet<ModelCategoryToken>>();
        (model_token, category_tokens)
      })
      .collect::<HashMap<TtsModelToken, HashSet<ModelCategoryToken>>>();

  Ok(map)
}
