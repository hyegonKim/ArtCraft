use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

// NB: This is "toggle" instead of discrete functions for create/delete because this is
// how old code was written and I'm lazy / in a hurry.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ToggleSoftDeleteState {
  Delete,
  Undelete,
}

pub async fn toggle_model_category_soft_delete(
  category_token: &str,
  state: ToggleSoftDeleteState,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
  // NB: We're soft deleting so we don't delete the associations.
  let query = match state {
    ToggleSoftDeleteState::Delete => {
      sqlx::query!(r#"
        UPDATE model_categories
        SET
          deleted_at = CURRENT_TIMESTAMP
        WHERE
          token = ?
        LIMIT 1
      "#,
      category_token)
    }
    ToggleSoftDeleteState::Undelete => {
      sqlx::query!(r#"
        UPDATE model_categories
        SET
          deleted_at = NULL
        WHERE
          token = ?
        LIMIT 1
      "#,
      category_token)
    }
  };

  // NB: We're soft deleting so we don't delete the associations.
  let query_result = query.execute(mysql_pool).await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
  }
}
