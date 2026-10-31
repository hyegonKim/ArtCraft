use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, NaiveDate, Utc};
use log::{error, info};
use sqlx::{Error, MySql, Pool};
use sqlx::pool::PoolConnection;
use tokio::time::Instant;

use datetimes::CHRONO_DATETIME_UNIX_EPOCH;
use datetimes::generate_dates_inclusive::generate_dates_inclusive;
use errors::AnyhowResult;
use mysql_queries::queries::model_weight_usage_counts::upsert_model_weight_usage_count_for_date::{Args, upsert_model_weight_usage_count_for_date};
use mysql_queries::queries::model_weights::count::count_model_use_using_media_files_on_date::count_model_use_using_media_files_on_date;
use mysql_queries::queries::model_weights::list::list_all_model_weight_tokens_for_backfill::list_all_model_weight_tokens_for_backfill;
use mysql_queries::queries::model_weights::list::list_model_weight_tokens_updated_since::list_model_weight_tokens_updated_since;
use tokens::tokens::media_files::MediaFileToken;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::operations::calculate_model_weights_usages::sub_args::{parse_cli_sub_args, SubArgs};

/// DO NOT USE THIS MIGRATION
/// IT IS SLOW AND INEFFICIENT
/// It does some nifty multithreaded and retry things, but the queries
/// themselves are super sparse and running the entire backfill will
/// take forever.
pub async fn run_migration_old(mysql: Pool<MySql>) -> AnyhowResult<()> {
  let args = parse_cli_sub_args()?;

  info!("Backfill args: {:?}", args);

  let models = match args.model_token.as_ref() {
    Some(model_token) => vec![ModelInfo {
      token: model_token.clone(),
      maybe_created_date: None,
    }],
    None => get_all_model_weight_tokens(mysql.clone()).await?,
  };

  info!("Model token count: {:?}", models.len());

  //for (i, model) in models.iter().enumerate() {
  //  let dates = get_all_dates(&args, model)?;
  //  for (j, date) in dates.iter().enumerate() {
  //    info!("Model: {}/{} Date: {}/{}", i + 1, models.len(), j + 1, dates.len());
  //    backfill_on_date_with_retry(&mut mysql.clone(), &mut connection, model, *date).await;
  //  }
  //}

  const THREAD_COUNT : usize = 8;
  let chunk_size = models.len() / THREAD_COUNT;

  let model_chunks = split_vec(models, chunk_size);
  let mut join_handles = Vec::with_capacity(model_chunks.len());

  for (h, model_chunk) in model_chunks.into_iter().enumerate() {
    let mysql_pool = mysql.clone();
    let args_copy = args.clone();

    let join = tokio::task::spawn(async move {
      let mut connection = match mysql_pool.acquire().await {
        Ok(cnx) => cnx,
        Err(err) => {
          error!("ERROR ACQUIRING FIRST MYSQL POOL CONNECTION FOR THREAD {}: {:?}", (h+1), err);
          return;
        }
      };

      for (i, model) in model_chunk.iter().enumerate() {
        let dates = get_all_dates(&args_copy, model)
            .unwrap_or_else(|_err| Vec::new());
        for (j, date) in dates.iter().enumerate() {
          info!("Thread: {}/{} Model: {}/{} Date: {}/{}",
            h + 1, THREAD_COUNT,
            i + 1, model_chunk.len(),
            j + 1, dates.len()
          );
          backfill_on_date_with_retry(&mut mysql_pool.clone(), &mut connection, model, *date).await;
        }
      }
    });
    join_handles.push(join);
  }

  let _r = futures::future::join_all(join_handles).await;

  Ok(())
}

async fn backfill_on_date_with_retry(
  mysql: &mut Pool<MySql>,
  connection: &mut PoolConnection<MySql>,
  model: &ModelInfo,
  date: NaiveDate,
) {
  let mut duration = Duration::from_secs(60);

  loop {
    let start = Instant::now();

    if let Err(err) = backfill_token(connection, &model.token, date).await {
      error!("Error backfilling token: {} for date: {:?} - {:?}", model.token.as_str(), date, err);
      tokio::time::sleep(Duration::from_secs(15)).await;

      error!("Re-acquire connection...");
      loop {
        match mysql.acquire().await {
          Err(err) => {
            error!("ERROR RE-ACQUIRING CONNECTION: {:?}", err);
            tokio::time::sleep(Duration::from_secs(15)).await;
          }
          Ok(cnx) => {
            *connection = cnx;
            break;
          }
        }
      }
      continue; // retry
    }

    duration = Instant::now().duration_since(start);
    break;
  }

  // Success, but add some backoff if we're stressing the database.
  if duration.as_millis() > 1600 {
    info!("Slow backfills. Slowing down queries...");
    tokio::time::sleep(Duration::from_secs(2)).await;
  }
}

pub struct ModelInfo {
  pub token: ModelWeightToken,
  pub maybe_created_date: Option<NaiveDate>,
}

async fn get_all_model_weight_tokens(mysql: Pool<MySql>) -> AnyhowResult<Vec<ModelInfo>> {
  let epoch = *CHRONO_DATETIME_UNIX_EPOCH;
  let results = list_all_model_weight_tokens_for_backfill(&mysql, &epoch).await?;
  Ok(results.into_iter()
      .map(|result| ModelInfo {
        token: result.token,
        maybe_created_date: Some(result.created_at.date_naive()),
      })
      .collect())
}

fn get_all_dates(args: &SubArgs, model: &ModelInfo) -> AnyhowResult<Vec<NaiveDate>> {
  let start_date = args.start_date
      .or_else(|| model.maybe_created_date)
      .ok_or_else(|| anyhow!("no start date given"))?;
  let end_date = args.end_date
      .unwrap_or_else(|| Utc::today().naive_utc());
  Ok(generate_dates_inclusive(start_date, end_date))
}

// NB: This methodology might be inaccurate
// We wrote to the media_files table (first record 2023-09-15 12:43:36) months after
// the model_weights table started to become populated (first record 2023-04-26 16:58:09).
// This means we might be missing inference generations.
async fn backfill_token(mysql: &mut PoolConnection<MySql>, model_token: &ModelWeightToken, date: NaiveDate) -> AnyhowResult<()> {
  info!("Backfilling token: {} for date: {:?}", model_token.as_str(), date);

  let count = count_model_use_using_media_files_on_date(
    &mut **mysql, &model_token, date).await?;

  if count.record_count == 0 {
    info!("Count: {:?} (skipping)", count.record_count);
    return Ok(());
  } else {
    info!("Count: {:?}", count.record_count);
  }

  upsert_model_weight_usage_count_for_date(Args {
    model_token: &model_token,
    date,
    usage_count: count.record_count,
    insert_on_zero: false,
    mysql_executor: &mut **mysql,
    phantom: Default::default(),
  }).await?;

  Ok(())
}

fn split_vec<T>(v: Vec<T>, chunk_size: usize) -> Vec<Vec<T>> {
  use std::collections::VecDeque;

  let mut v: VecDeque<T> = v.into(); // avoids reallocating when possible

  let mut acc = Vec::new();
  while v.len() > chunk_size {
    acc.push(v.drain(0..chunk_size).collect());
    v.shrink_to_fit();
  }
  acc.push(v.into());
  acc
}
