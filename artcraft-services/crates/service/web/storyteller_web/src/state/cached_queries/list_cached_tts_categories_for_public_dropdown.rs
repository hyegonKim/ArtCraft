use anyhow::anyhow;
use lexical_sort::natural_lexical_cmp;
use log::error;
use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use memory_caching::single_item_ttl_cache::SingleItemTtlCache;
use mysql_queries::queries::model_categories::list_categories_query_builder::{CategoryList, ListCategoriesQueryBuilder};

/// Fetch from cache if available, otherwise fetch from DB.
pub async fn list_cached_tts_categories_for_public_dropdown_db_pool(
  cache: &SingleItemTtlCache<CategoryList>,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<CategoryList> {
  let mut mysql_connection = mysql_pool.acquire().await?;
  list_cached_tts_categories_for_public_dropdown(cache, &mut mysql_connection).await
}

/// Fetch from cache if available, otherwise fetch from DB.
pub async fn list_cached_tts_categories_for_public_dropdown(
  cache: &SingleItemTtlCache<CategoryList>,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<CategoryList> {
  let maybe_database_categories = cache.grab_copy_without_bump_if_unexpired()?;

  match maybe_database_categories {
    Some(categories) => Ok(categories),
    None => {
      // NB: Scope to just public TTS categories used in the dropdown
      let query_builder = ListCategoriesQueryBuilder::new()
          .show_deleted(false)
          .show_unapproved(false)
          .scope_model_type(Some("tts"));

      let query_result =
          query_builder.perform_query_using_connection(mysql_connection).await;

      let mut results : CategoryList = match query_result {
        Ok(results) => results,
        Err(err) => {
          error!("DB error: {:?}", err);
          return Err(anyhow!("database error"));
        }
      };

      // NB: This might produce weird sorting resorts relative to the "name" field,
      // but the typical way this should be consumed is via dropdowns.
      results.categories.sort_by(|c1, c2|
          natural_lexical_cmp(
            &c1.maybe_dropdown_name.as_deref().unwrap_or(&c1.name),
            &c2.maybe_dropdown_name.as_deref().unwrap_or(&c2.name),
          ));

      cache.store_copy(&results)
          .map_err(|e| {
            error!("Error storing cache: {:?}", e);
            anyhow!("cache save error")
          })?;

      Ok(results)
    },
  }
}
