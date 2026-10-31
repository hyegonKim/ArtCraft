use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize)]
pub struct StaffRecordForList {
  pub user_token: String,
  pub username: String,
  pub display_name: String,
  pub user_role_slug: String,
  pub user_role_name: String,
}

pub async fn list_staff(mysql_pool: &MySqlPool) -> AnyhowResult<Vec<StaffRecordForList>> {
  // NB: Lookup failure is Err(RowNotFound).
  let maybe_results = sqlx::query_as!(
      StaffRecordForList,
        r#"
SELECT
    users.token as user_token,
    users.username,
    users.display_name,
    user_roles.slug as user_role_slug,
    user_roles.name as user_role_name
FROM
    users
JOIN user_roles
    ON users.user_role_slug = user_roles.slug
WHERE
    user_roles.slug != 'user'
        "#,
    )
      .fetch_all(mysql_pool)
      .await;

  match maybe_results {
    Ok(results) => Ok(results),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(Vec::new()),
      _ => Err(anyhow!("Error with query: {:?}", err)),
    }
  }
}
