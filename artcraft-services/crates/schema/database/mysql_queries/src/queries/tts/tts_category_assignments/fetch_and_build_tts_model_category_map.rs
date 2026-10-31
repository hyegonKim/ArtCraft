// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use log::warn;
use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

/// Map of model_token => vec<category tokens>
#[derive(Serialize, Default)]
pub struct TtsModelCategoryMap {
  pub model_to_category_tokens: HashMap<String, HashSet<String>>,
}

/// Fetch a map of every model to all of its categories.
pub async fn fetch_and_build_tts_model_category_map(
  mysql_pool: &MySqlPool
) -> AnyhowResult<TtsModelCategoryMap> {
  let mut connection = mysql_pool.acquire().await?;
  fetch_and_build_tts_model_category_map_with_connection(&mut connection).await
}

/// Fetch a map of every model to all of its categories.
pub async fn fetch_and_build_tts_model_category_map_with_connection(
  mysql_connection: &mut PoolConnection<MySql>
) -> AnyhowResult<TtsModelCategoryMap> {
  let assignments = list_tts_model_category_assignments(mysql_connection).await?;

  let mut map : HashMap<String, HashSet<String>> = HashMap::new();

  for assignment in assignments.iter() {
    if !map.contains_key(&assignment.tts_model_token) {
      map.insert(assignment.tts_model_token.clone(), HashSet::new());
    }

    map.get_mut(&assignment.tts_model_token).map(|hashset| {
      hashset.insert(assignment.category_token.clone())
    });
  }

  Ok(TtsModelCategoryMap {
    model_to_category_tokens: map,
  })
}

#[derive(Serialize)]
pub struct CategoryAssignment {
  pub tts_model_token: String,
  pub category_token: String,
}

async fn list_tts_model_category_assignments(mysql_connection: &mut PoolConnection<MySql>) -> AnyhowResult<Vec<CategoryAssignment>> {
  let maybe_results = sqlx::query_as!(
      CategoryAssignment,
        r#"
SELECT
    assignments.model_token AS tts_model_token,
    assignments.category_token
FROM
    tts_category_assignments AS assignments
JOIN
    tts_models AS tts
    ON assignments.model_token = tts.token
WHERE
    tts.is_locked_from_use IS FALSE
    AND tts.user_deleted_at IS NULL
    AND tts.mod_deleted_at IS NULL
    AND assignments.deleted_at IS NULL

        "#,
    )
      .fetch_all(&mut **mysql_connection)
      .await;

  match maybe_results {
    Err(err) => {
      match err {
        sqlx::Error::RowNotFound => {
          Ok(Vec::new())
        },
        _ => {
          warn!("list tts model category assignments db error: {:?}", err);
          Err(anyhow!("error with query: {:?}", err))
        }
      }
    },
    Ok(results) => Ok(results),
  }
}
