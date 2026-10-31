use anyhow::anyhow;
use log::error;
use sqlx::{MySql, QueryBuilder, Transaction};

use composite_identifiers::by_table::batch_generations::batch_generation_entity::BatchGenerationEntity;
use errors::AnyhowResult;
use tokens::tokens::batch_generations::BatchGenerationToken;

pub struct BatchEntry {
  pub batch_token: BatchGenerationToken,
  pub entity: BatchGenerationEntity,
}

pub struct InsertBatchArgs<'a, 'b> {
  pub entries: Vec<BatchGenerationEntity>,
  pub maybe_existing_batch_token: Option<&'a BatchGenerationToken>,
  pub transaction: &'a mut Transaction<'b, MySql>,
}

/// Insert a list of entities into a "batch" together for grouping; returns the new batch token identifier to return
/// to the HTTP caller.
///
/// NB: Calling code is responsible for rolling back the transaction if this fails.
pub async fn insert_batch_generation_records<'a, 'b>(args: InsertBatchArgs<'a, 'b>) -> AnyhowResult<BatchGenerationToken> {

  let batch_token = match args.maybe_existing_batch_token {
    Some(existing_token) => existing_token.clone(),
    None => BatchGenerationToken::generate(),
  };

  let mut query_builder = QueryBuilder::new(r#"
    INSERT INTO batch_generations (token, entity_type, entity_token) VALUES
  "#);

  for (i, entry) in args.entries.iter().enumerate() {
    let (entity_type, entity_token) = entry.get_composite_keys();

    query_builder.push("(");
    query_builder.push_bind(&batch_token);
    query_builder.push(",");

    query_builder.push_bind(entity_type);
    query_builder.push(",");

    query_builder.push_bind(entity_token);
    query_builder.push(")");

    if i < args.entries.len() - 1 {
      query_builder.push(",");
    }
  }

  let query = query_builder.build();

  let query_result  = query.execute(&mut **args.transaction).await;

  match query_result {
    Ok(_) => Ok(batch_token),
    Err(err) => {
      error!("Error with batch generation query: {:?}", &err);
      Err(anyhow!("model category creation error: {:?}", &err))
    },
  }
}

#[ignore]
#[cfg(test)]
mod tests {
  use sqlx::mysql::MySqlPoolOptions;

  use composite_identifiers::by_table::batch_generations::batch_generation_entity::BatchGenerationEntity;
  use tokens::tokens::media_files::MediaFileToken;

  use crate::config::shared_constants::DEFAULT_MYSQL_CONNECTION_STRING;
  use crate::queries::batch_generations::insert_batch_generation_records::{insert_batch_generation_records, InsertBatchArgs};

  async fn setup() -> sqlx::Pool<sqlx::MySql> {
    MySqlPoolOptions::new()
        .max_connections(3)
        .connect(&DEFAULT_MYSQL_CONNECTION_STRING).await
        .unwrap()
  }

  #[ignore]
  #[tokio::test]
  async fn test() {
    let pool = setup().await;

    let mut transaction = pool.begin().await.unwrap();

    let entries = vec![
      BatchGenerationEntity::MediaFile(MediaFileToken::new_from_str("media_foo")),
      BatchGenerationEntity::MediaFile(MediaFileToken::new_from_str("media_bar")),
      BatchGenerationEntity::MediaFile(MediaFileToken::new_from_str("media_baz")),
    ];

    let r = insert_batch_generation_records(InsertBatchArgs {
      entries,
      maybe_existing_batch_token: None,
      transaction: &mut transaction,
    }).await;

    transaction.commit().await.unwrap();
  }
}