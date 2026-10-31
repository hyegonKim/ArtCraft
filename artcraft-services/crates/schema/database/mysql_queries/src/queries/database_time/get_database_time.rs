use anyhow::anyhow;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct GetDatabaseTimeResult {
  pub database_time: DateTime<Utc>,
}

struct InternalGetDatabaseTimeResult {
  database_time: NaiveDateTime,
}

/// Query the DB for its current time
pub async fn get_database_time(
  pool: &MySqlPool,
) -> AnyhowResult<GetDatabaseTimeResult> {

  let maybe_record = sqlx::query_as!(
      InternalGetDatabaseTimeResult,
        r#"
SELECT NOW() as database_time
        "#,
    )
      .fetch_one(pool)
      .await;

  match maybe_record {
    Ok(record) => Ok(GetDatabaseTimeResult {
      database_time: record.database_time.and_utc(),
    }),
    Err(sqlx::Error::RowNotFound) => Err(anyhow!("query didn't return anything")),
    Err(ref err) => Err(anyhow!("database query error: {:?}", err)),
  }
}
