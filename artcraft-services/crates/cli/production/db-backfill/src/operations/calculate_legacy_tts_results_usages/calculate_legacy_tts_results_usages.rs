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
use mysql_queries::queries::model_weights::count::count_all_legacy_model_usages_on_date::count_all_legacy_model_usages_on_date;
use mysql_queries::queries::model_weights::count::count_all_model_usages_on_date::count_all_model_usages_on_date;
use mysql_queries::queries::model_weights::count::count_model_use_using_media_files_on_date::count_model_use_using_media_files_on_date;
use mysql_queries::queries::model_weights::list::list_all_model_weight_tokens_for_backfill::list_all_model_weight_tokens_for_backfill;
use mysql_queries::queries::model_weights::list::list_model_weight_tokens_updated_since::list_model_weight_tokens_updated_since;
use tokens::tokens::media_files::MediaFileToken;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::operations::calculate_legacy_tts_results_usages::sub_args::parse_cli_sub_args;

pub async fn calculate_legacy_tts_result_usages(mysql: Pool<MySql>) -> AnyhowResult<()> {
  let args = parse_cli_sub_args()?;

  info!("Grab mysql connection...");
  let mut connection = mysql.acquire().await?;

  info!("Backfill args: {:?}", args);

  let start_date = args.start_date.unwrap_or(Utc::today().naive_utc());
  let end_date = args.end_date.unwrap_or(Utc::today().naive_utc());

  let dates = generate_dates_inclusive(start_date, end_date);

  let start = Instant::now();

  for date in dates {
    info!("Finding usages for date: {:?}", date);

    let usages = count_all_legacy_model_usages_on_date(&mut *connection, date).await?;

    info!("Records found: {}", usages.counts.len());

    for usage in usages.counts {
      if usage.record_count == 0 {
        continue;
      }

      info!("Date: {} Token: {} Uses: {} (batch elapsed: {} seconds)", date, usage.token.as_str(), usage.record_count, start.elapsed().as_secs());

      upsert_model_weight_usage_count_for_date(Args {
        model_token: &usage.token,
        date,
        usage_count: usage.record_count,
        insert_on_zero: false,
        mysql_executor: &mut *connection,
        phantom: Default::default(),
      }).await?;
    }
  }

  Ok(())
}
